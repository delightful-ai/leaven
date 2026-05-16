//! Codex event normalization.

#![cfg(feature = "app-server")]

use codex_app_server_protocol::{
    JSONRPCNotification, ServerNotification, Thread, ThreadItem, Turn, TurnCompletedNotification,
    TurnStartedNotification, TurnStatus, UserInput,
};
use leaven_agent::{
    AgentSession, AgentStatus, RawProviderEvent, ToolCallRecord, TranscriptEvent, TranscriptRole,
};
use leaven_kernel::AgentSessionId;

use crate::config::CodexRawEventPolicy;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CodexHistory {
    thread_id: String,
    turns: Vec<CodexTurnHistory>,
    warnings: Vec<String>,
    raw_events: Vec<RawProviderEvent>,
}

impl CodexHistory {
    pub(crate) fn new(thread_id: impl Into<String>) -> Self {
        Self {
            thread_id: thread_id.into(),
            turns: Vec::new(),
            warnings: Vec::new(),
            raw_events: Vec::new(),
        }
    }

    pub(crate) fn record_thread(&mut self, thread: &Thread) {
        self.thread_id.clone_from(&thread.id);
        for turn in &thread.turns {
            self.record_turn(turn);
        }
    }

    pub(crate) fn record_notification(
        &mut self,
        notification: &ServerNotification,
        raw_event_policy: CodexRawEventPolicy,
    ) {
        if raw_event_policy.retains() {
            self.raw_events.push(raw_provider_event(notification));
        }

        match notification {
            ServerNotification::ThreadStarted(payload) => {
                self.thread_id.clone_from(&payload.thread.id);
                self.record_thread(&payload.thread);
            }
            ServerNotification::TurnStarted(payload) => self.record_turn_started(payload),
            ServerNotification::TurnCompleted(payload) => self.record_turn_completed(payload),
            ServerNotification::ItemStarted(payload) => {
                self.ensure_turn(&payload.turn_id)
                    .record_item(&payload.item);
            }
            ServerNotification::ItemCompleted(payload) => {
                self.ensure_turn(&payload.turn_id)
                    .record_item(&payload.item);
            }
            ServerNotification::AgentMessageDelta(delta) => {
                self.ensure_turn(&delta.turn_id).record_text_delta(
                    &delta.item_id,
                    CodexTextRole::Assistant,
                    &delta.delta,
                );
            }
            ServerNotification::PlanDelta(delta) => {
                self.ensure_turn(&delta.turn_id).record_text_delta(
                    &delta.item_id,
                    CodexTextRole::Plan,
                    &delta.delta,
                );
            }
            ServerNotification::ReasoningSummaryTextDelta(delta) => {
                self.ensure_turn(&delta.turn_id).record_text_delta(
                    &delta.item_id,
                    CodexTextRole::Reasoning,
                    &delta.delta,
                );
            }
            ServerNotification::ReasoningTextDelta(delta) => {
                self.ensure_turn(&delta.turn_id).record_text_delta(
                    &delta.item_id,
                    CodexTextRole::Reasoning,
                    &delta.delta,
                );
            }
            ServerNotification::CommandExecutionOutputDelta(delta) => {
                self.ensure_turn(&delta.turn_id)
                    .record_command_delta(&delta.item_id, &delta.delta);
            }
            ServerNotification::Error(payload) => {
                self.ensure_turn(&payload.turn_id)
                    .errors
                    .push(payload.error.message.clone());
            }
            ServerNotification::Warning(payload) => self.warnings.push(payload.message.clone()),
            ServerNotification::GuardianWarning(payload) => {
                self.warnings.push(payload.message.clone());
            }
            ServerNotification::DeprecationNotice(payload) => {
                self.warnings.push(payload.summary.clone());
            }
            _ => {}
        }
    }

    pub(crate) fn record_raw_notification(
        &mut self,
        notification: &JSONRPCNotification,
        error: &str,
        raw_event_policy: CodexRawEventPolicy,
    ) {
        if raw_event_policy.retains() {
            self.raw_events
                .push(raw_jsonrpc_notification_event(notification));
        }
        self.warnings.push(format!(
            "skipped Codex notification `{}`: {error}",
            notification.method
        ));
    }

    pub(crate) fn into_agent_session(
        self,
        session_id: AgentSessionId,
        terminal_turn: &Turn,
    ) -> AgentSession {
        let mut session = AgentSession::succeeded(session_id);
        session.status = agent_status(terminal_turn, &self.warnings);
        session.raw_provider_events = self.raw_events;

        for turn in self.turns {
            for user_message in turn.user_messages {
                session
                    .transcript
                    .push_message(TranscriptRole::User, user_message);
            }
            for reasoning in turn.reasoning {
                session
                    .transcript
                    .push_message(TranscriptRole::Tool, format!("reasoning: {reasoning}"));
            }
            for plan in turn.plans {
                session
                    .transcript
                    .push_message(TranscriptRole::Tool, format!("plan: {plan}"));
            }
            for command in turn.commands {
                session.transcript.events.push(TranscriptEvent::ToolCall {
                    record: ToolCallRecord {
                        name: "codex.command".to_owned(),
                        input: command.command,
                        output: Some(command.output),
                    },
                });
            }
            for assistant_message in turn.assistant_messages {
                session
                    .transcript
                    .push_message(TranscriptRole::Assistant, assistant_message);
            }
            for error in turn.errors {
                session
                    .transcript
                    .push_message(TranscriptRole::Tool, format!("error: {error}"));
            }
        }

        session
    }

    fn record_turn_started(&mut self, payload: &TurnStartedNotification) {
        self.record_turn(&payload.turn);
    }

    fn record_turn_completed(&mut self, payload: &TurnCompletedNotification) {
        self.record_turn(&payload.turn);
    }

    fn record_turn(&mut self, turn: &Turn) {
        let record = self.ensure_turn(&turn.id);
        record.status = turn_status_name(&turn.status);
        if let Some(error) = &turn.error {
            push_unique(&mut record.errors, error.message.clone());
        }
        for item in &turn.items {
            record.record_item(item);
        }
    }

    fn ensure_turn(&mut self, turn_id: &str) -> &mut CodexTurnHistory {
        if let Some(index) = self.turns.iter().position(|turn| turn.turn_id == turn_id) {
            return &mut self.turns[index];
        }

        self.turns.push(CodexTurnHistory {
            turn_id: turn_id.to_owned(),
            status: "in_progress".to_owned(),
            ..CodexTurnHistory::default()
        });
        self.turns.last_mut().expect("turn was just pushed")
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct CodexTurnHistory {
    turn_id: String,
    status: String,
    user_messages: Vec<String>,
    assistant_messages: Vec<String>,
    reasoning: Vec<String>,
    plans: Vec<String>,
    commands: Vec<CodexCommandItem>,
    errors: Vec<String>,
}

impl CodexTurnHistory {
    fn record_item(&mut self, item: &ThreadItem) {
        match item {
            ThreadItem::UserMessage { content, .. } => {
                for text in user_input_text(content) {
                    push_unique(&mut self.user_messages, text);
                }
            }
            ThreadItem::AgentMessage { text, .. } => {
                push_unique(&mut self.assistant_messages, text.clone());
            }
            ThreadItem::Plan { text, .. } => {
                push_unique(&mut self.plans, text.clone());
            }
            ThreadItem::Reasoning {
                summary, content, ..
            } => {
                let text = summary
                    .iter()
                    .chain(content.iter())
                    .cloned()
                    .collect::<Vec<_>>()
                    .join("\n");
                if !text.trim().is_empty() {
                    push_unique(&mut self.reasoning, text);
                }
            }
            ThreadItem::CommandExecution {
                id,
                command,
                aggregated_output,
                ..
            } => {
                upsert_command(
                    &mut self.commands,
                    CodexCommandItem {
                        item_id: id.clone(),
                        command: command.clone(),
                        output: aggregated_output.clone().unwrap_or_default(),
                    },
                );
            }
            ThreadItem::McpToolCall {
                server,
                tool,
                error,
                ..
            } => {
                let summary = error
                    .as_ref()
                    .map_or_else(|| "ok".to_owned(), |error| format!("{error:?}"));
                push_unique(
                    &mut self.plans,
                    format!("mcp tool call {server}/{tool}: {summary}"),
                );
            }
            ThreadItem::DynamicToolCall {
                namespace,
                tool,
                success,
                ..
            } => {
                push_unique(
                    &mut self.plans,
                    format!(
                        "dynamic tool call {}/{}: {}",
                        namespace.as_deref().unwrap_or("default"),
                        tool,
                        success.unwrap_or(false)
                    ),
                );
            }
            ThreadItem::FileChange { changes, .. } => {
                push_unique(&mut self.plans, format!("file changes: {changes:?}"));
            }
            ThreadItem::WebSearch { query, .. } => {
                push_unique(&mut self.plans, format!("web search: {query}"));
            }
            _ => {}
        }
    }

    fn record_text_delta(&mut self, item_id: &str, role: CodexTextRole, delta: &str) {
        let target = match role {
            CodexTextRole::Assistant => &mut self.assistant_messages,
            CodexTextRole::Reasoning => &mut self.reasoning,
            CodexTextRole::Plan => &mut self.plans,
        };
        if let Some(existing) = target.last_mut() {
            existing.push_str(delta);
            return;
        }
        target.push(format!("{item_id}: {delta}"));
    }

    fn record_command_delta(&mut self, item_id: &str, delta: &str) {
        if let Some(command) = self
            .commands
            .iter_mut()
            .find(|command| command.item_id == item_id)
        {
            command.output.push_str(delta);
            return;
        }
        self.commands.push(CodexCommandItem {
            item_id: item_id.to_owned(),
            command: String::new(),
            output: delta.to_owned(),
        });
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CodexCommandItem {
    item_id: String,
    command: String,
    output: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CodexTextRole {
    Assistant,
    Reasoning,
    Plan,
}

fn user_input_text(content: &[UserInput]) -> Vec<String> {
    content
        .iter()
        .map(|input| match input {
            UserInput::Text { text, .. } => text.clone(),
            UserInput::Image { url, .. } => format!("[image: {url}]"),
            UserInput::LocalImage { path, .. } => format!("[local image: {}]", path.display()),
            UserInput::Skill { name, path } => {
                format!("[skill: {name} at {}]", path.display())
            }
            UserInput::Mention { name, path } => format!("[mention: {name} at {path}]"),
        })
        .collect()
}

fn turn_status_name(status: &TurnStatus) -> String {
    match status {
        TurnStatus::Completed => "completed",
        TurnStatus::Interrupted => "interrupted",
        TurnStatus::Failed => "failed",
        TurnStatus::InProgress => "in_progress",
    }
    .to_owned()
}

fn agent_status(turn: &Turn, warnings: &[String]) -> AgentStatus {
    match turn.status {
        TurnStatus::Completed if turn.error.is_none() => AgentStatus::Succeeded,
        TurnStatus::Interrupted => AgentStatus::Cancelled,
        TurnStatus::Failed => AgentStatus::Failed {
            reason: turn.error.as_ref().map_or_else(
                || "Codex turn failed".to_owned(),
                |error| error.message.clone(),
            ),
        },
        TurnStatus::InProgress => AgentStatus::Failed {
            reason: "Codex turn ended while still in progress".to_owned(),
        },
        TurnStatus::Completed => AgentStatus::Failed {
            reason: turn
                .error
                .as_ref()
                .map_or_else(|| warnings.join("\n"), |error| error.message.clone()),
        },
    }
}

fn raw_provider_event(notification: &ServerNotification) -> RawProviderEvent {
    RawProviderEvent {
        kind: notification_method(notification),
        payload: serde_json::to_string(notification).unwrap_or_else(|error| {
            format!(
                "{{\"serialization_error\":{}}}",
                serde_json::Value::String(error.to_string())
            )
        }),
    }
}

fn raw_jsonrpc_notification_event(notification: &JSONRPCNotification) -> RawProviderEvent {
    RawProviderEvent {
        kind: notification.method.clone(),
        payload: serde_json::to_string(notification).unwrap_or_else(|error| {
            format!(
                "{{\"serialization_error\":{}}}",
                serde_json::Value::String(error.to_string())
            )
        }),
    }
}

fn notification_method(notification: &ServerNotification) -> String {
    serde_json::to_value(notification)
        .ok()
        .and_then(|value| {
            value
                .get("method")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        })
        .unwrap_or_else(|| format!("{notification:?}"))
}

fn push_unique(target: &mut Vec<String>, value: String) {
    if !target.contains(&value) {
        target.push(value);
    }
}

fn upsert_command(target: &mut Vec<CodexCommandItem>, value: CodexCommandItem) {
    if let Some(existing) = target
        .iter_mut()
        .find(|command| command.item_id == value.item_id)
    {
        *existing = value;
        return;
    }
    target.push(value);
}
