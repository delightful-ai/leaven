//! `AgentRuntime` implementation over Codex app-server.

#![cfg(feature = "app-server")]

use std::path::Path;

use codex_app_server_protocol::{
    ThreadReadParams, ThreadStartParams, Turn, TurnStartParams, UserInput,
};
use leaven_agent::{
    AgentInstructions, AgentRunContext, AgentRunRequest, AgentRuntime, AgentRuntimeCapabilities,
    AgentRuntimeError, AgentSession, AgentStatus, OutputContract, TranscriptEvent, TranscriptRole,
    WorkspaceAccessMode, validate_output_contract,
};
use leaven_kernel::{AgentRuntimeId, Cost, Fingerprint, FingerprintBuilder, Metered};
use leaven_workspace::WorkspaceView;

use crate::client::{CodexAppServerClient, InitializeOptions};
use crate::config::{CodexAppServerConfig, CodexRawEventPolicy};
use crate::error::{CodexAppServerError, Result as CodexResult};
use crate::history::CodexHistory;
use crate::transport::CodexAppServerConnector;

#[derive(Clone, Debug)]
pub struct CodexAppServerRuntime<Connector> {
    config: CodexAppServerConfig,
    connector: Connector,
}

impl<Connector> CodexAppServerRuntime<Connector> {
    #[must_use]
    pub fn new(config: CodexAppServerConfig, connector: Connector) -> Self {
        Self { config, connector }
    }

    #[must_use]
    pub const fn config(&self) -> &CodexAppServerConfig {
        &self.config
    }

    #[must_use]
    pub const fn connector(&self) -> &Connector {
        &self.connector
    }
}

#[cfg(feature = "stdio")]
impl Default for CodexAppServerRuntime<crate::StdioCodexAppServerConnector> {
    fn default() -> Self {
        Self::new(
            CodexAppServerConfig::default(),
            crate::StdioCodexAppServerConnector::default(),
        )
    }
}

impl<Connector> AgentRuntime for CodexAppServerRuntime<Connector>
where
    Connector: CodexAppServerConnector,
{
    fn id(&self) -> AgentRuntimeId {
        AgentRuntimeId::new_const("codex-app-server")
    }

    fn fingerprint(&self) -> Fingerprint {
        let mut builder = FingerprintBuilder::new();
        builder.update("codex-app-server-runtime/v1");
        builder.update(self.config.fingerprint().0);
        self.connector.feed_fingerprint(&mut builder);
        builder.finish()
    }

    fn capabilities(&self) -> AgentRuntimeCapabilities {
        AgentRuntimeCapabilities {
            workspace_access: self.connector.workspace_access(),
            supports_commands: true,
            supports_raw_provider_events: self.config.retain_raw_events.retains(),
        }
    }

    async fn run_session(
        &self,
        workspace: &mut WorkspaceView<'_>,
        request: AgentRunRequest,
        ctx: AgentRunContext<'_>,
    ) -> std::result::Result<Metered<AgentSession>, AgentRuntimeError> {
        if ctx.cancellation().is_cancelled() {
            let mut session = AgentSession::succeeded(ctx.session_id());
            session.status = AgentStatus::Cancelled;
            return Ok(Metered::new(session, Cost::zero()));
        }

        if self.connector.workspace_access() == WorkspaceAccessMode::RequiresLocalMount
            && workspace.local_mount().is_none()
        {
            return Err(AgentRuntimeError::LocalMountRequired { runtime: self.id() });
        }

        let session_id = ctx.session_id();
        let timeout = request.limits.timeout;
        let output_contract = request.output_contract.clone();
        let system = request.instructions.system.clone();
        let run = self.run_session_once(workspace, request, session_id);

        let mut metered = if let Some(timeout) = timeout {
            if let Ok(result) = tokio::time::timeout(timeout, run).await {
                result?
            } else {
                let mut session = AgentSession::succeeded(session_id);
                session.status = AgentStatus::TimedOut;
                return Ok(Metered::new(session, Cost::zero()));
            }
        } else {
            run.await?
        };

        if let Some(system) = system {
            metered.value.transcript.events.insert(
                0,
                TranscriptEvent::Message {
                    role: TranscriptRole::System,
                    content: system,
                },
            );
        }

        for path in validate_output_contract(workspace, &output_contract, &metered.value)? {
            if !metered.value.output_files.contains(&path) {
                metered.value.output_files.push(path);
            }
        }

        Ok(metered)
    }
}

impl<Connector> CodexAppServerRuntime<Connector>
where
    Connector: CodexAppServerConnector,
{
    async fn run_session_once(
        &self,
        workspace: &WorkspaceView<'_>,
        request: AgentRunRequest,
        session_id: leaven_kernel::AgentSessionId,
    ) -> std::result::Result<Metered<AgentSession>, AgentRuntimeError> {
        let connection = self
            .connector
            .connect(workspace, &request)
            .await
            .map_err(map_runtime_error)?;
        let app_server_cwd = connection.cwd;
        let mut client = CodexAppServerClient::new(connection.transport)
            .with_approval_mode(self.config.approval_mode.into());

        client
            .initialize(InitializeOptions {
                client_name: self.config.initialize.client_name.clone(),
                client_title: self.config.initialize.client_title.clone(),
                experimental_api: self.config.initialize.experimental_api,
                opt_out_notification_methods: self
                    .config
                    .initialize
                    .opt_out_notification_methods
                    .clone(),
            })
            .await
            .map_err(map_runtime_error)?;

        let thread = client
            .thread_start(thread_start_params(&self.config, &app_server_cwd))
            .await
            .map_err(map_runtime_error)?;
        let thread_id = thread.thread.id.clone();
        let thread_is_ephemeral = thread.thread.ephemeral;
        let mut history = CodexHistory::new(&thread_id);
        history.record_thread(&thread.thread);

        let turn = client
            .turn_start(
                turn_start_params(&self.config, &thread_id, &app_server_cwd, &request)
                    .map_err(map_runtime_error)?,
            )
            .await
            .map_err(map_runtime_error)?;
        history.record_notification(
            &codex_app_server_protocol::ServerNotification::TurnStarted(
                codex_app_server_protocol::TurnStartedNotification {
                    thread_id: thread_id.clone(),
                    turn: turn.turn.clone(),
                },
            ),
            self.config.retain_raw_events,
        );
        let turn = stream_until_turn_completed(
            &mut client,
            &mut history,
            &turn.turn.id,
            self.config.retain_raw_events,
        )
        .await
        .map_err(map_runtime_error)?;

        if matches!(
            turn.items_view,
            codex_app_server_protocol::TurnItemsView::NotLoaded
        ) && !thread_is_ephemeral
        {
            let refreshed = client
                .thread_read(ThreadReadParams {
                    thread_id,
                    include_turns: true,
                })
                .await
                .map_err(map_runtime_error)?;
            history.record_thread(&refreshed.thread);
        }

        let _ = client.shutdown().await;
        let session = history.into_agent_session(session_id, &turn);
        Ok(Metered::new(session, Cost::llm_calls(1)))
    }
}

async fn stream_until_turn_completed<T>(
    client: &mut CodexAppServerClient<T>,
    history: &mut CodexHistory,
    expected_turn_id: &str,
    raw_event_policy: CodexRawEventPolicy,
) -> CodexResult<Turn>
where
    T: crate::CodexAppServerTransport,
{
    loop {
        let raw_notification = client.next_raw_notification().await?;
        let notification =
            match codex_app_server_protocol::ServerNotification::try_from(raw_notification.clone())
            {
                Ok(notification) => notification,
                Err(error) => {
                    history.record_raw_notification(
                        &raw_notification,
                        &error.to_string(),
                        raw_event_policy,
                    );
                    continue;
                }
            };
        history.record_notification(&notification, raw_event_policy);
        if let codex_app_server_protocol::ServerNotification::TurnCompleted(payload) = notification
            && payload.turn.id == expected_turn_id
        {
            return Ok(payload.turn);
        }
    }
}

fn thread_start_params(config: &CodexAppServerConfig, cwd: &Path) -> ThreadStartParams {
    ThreadStartParams {
        model: config.thread.model.clone(),
        model_provider: config.thread.model_provider.clone(),
        service_tier: config.thread.service_tier.clone().map(Some),
        cwd: Some(cwd.display().to_string()),
        approval_policy: config.thread.approval_policy.map(Into::into),
        approvals_reviewer: config.thread.approvals_reviewer.map(Into::into),
        sandbox: config.thread.sandbox.map(Into::into),
        base_instructions: config.thread.base_instructions.clone(),
        developer_instructions: config.thread.developer_instructions.clone(),
        ephemeral: Some(config.thread.ephemeral),
        service_name: config.thread.service_name.clone(),
        experimental_raw_events: config.retain_raw_events.retains(),
        ..ThreadStartParams::default()
    }
}

fn turn_start_params(
    config: &CodexAppServerConfig,
    thread_id: &str,
    cwd: &Path,
    request: &AgentRunRequest,
) -> CodexResult<TurnStartParams> {
    Ok(TurnStartParams {
        thread_id: thread_id.to_owned(),
        input: vec![UserInput::Text {
            text: format_instructions(&request.instructions),
            text_elements: Vec::new(),
        }],
        cwd: Some(cwd.to_path_buf()),
        approval_policy: config.turn.approval_policy.map(Into::into),
        approvals_reviewer: config.turn.approvals_reviewer.map(Into::into),
        model: config.turn.model.clone(),
        service_tier: config.turn.service_tier.clone().map(Some),
        effort: config.turn.effort.map(Into::into),
        summary: config.turn.summary.map(Into::into),
        output_schema: output_schema(&request.output_contract)?,
        ..TurnStartParams::default()
    })
}

fn format_instructions(instructions: &AgentInstructions) -> String {
    let mut text = String::new();
    if let Some(system) = &instructions.system {
        text.push_str(system.trim());
        text.push_str("\n\n");
    }
    text.push_str(instructions.task.trim());
    if !instructions.context.is_empty() {
        text.push_str("\n\nContext files:\n");
        for context in &instructions.context {
            text.push_str("- ");
            text.push_str(&context.label);
            text.push_str(": ");
            text.push_str(context.path.as_str());
            if let Some(media_type) = &context.media_type {
                text.push_str(" (");
                text.push_str(media_type);
                text.push(')');
            }
            text.push('\n');
        }
    }
    text
}

fn output_schema(contract: &OutputContract) -> CodexResult<Option<serde_json::Value>> {
    match contract {
        OutputContract::JsonFile {
            schema: Some(schema),
            ..
        } => serde_json::from_str(&schema.schema)
            .map(Some)
            .map_err(CodexAppServerError::from),
        OutputContract::Files { .. }
        | OutputContract::JsonFile { schema: None, .. }
        | OutputContract::FinalMessage
        | OutputContract::WorkspaceDiff { .. } => Ok(None),
    }
}

fn map_runtime_error(error: CodexAppServerError) -> AgentRuntimeError {
    AgentRuntimeError::with_source("codex app-server runtime failed", error)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use codex_app_server_protocol::{
        ItemCompletedNotification, JSONRPCMessage, JSONRPCResponse, RequestId, ServerNotification,
        ThreadStartResponse, TurnCompletedNotification,
    };
    use leaven_agent::{AgentInstructions, OutputContract};
    use leaven_kernel::{AgentSessionId, BudgetSnapshot};
    use leaven_workspace::WorkspaceFactory;
    use leaven_workspace_local::LocalWorkspaceFactory;

    use super::*;
    use crate::transport::CodexAppServerConnection;
    use crate::transport::tests::MockTransport;

    #[derive(Clone)]
    struct MockConnector {
        inbound: Vec<String>,
        workspace_access: WorkspaceAccessMode,
    }

    impl CodexAppServerConnector for MockConnector {
        type Transport = MockTransport;

        fn workspace_access(&self) -> WorkspaceAccessMode {
            self.workspace_access.clone()
        }

        fn feed_fingerprint(&self, builder: &mut FingerprintBuilder) {
            builder.update("mock-connector");
        }

        async fn connect(
            &self,
            _workspace: &WorkspaceView<'_>,
            _request: &AgentRunRequest,
        ) -> CodexResult<CodexAppServerConnection<Self::Transport>> {
            Ok(CodexAppServerConnection {
                transport: MockTransport::new(self.inbound.clone()),
                cwd: PathBuf::from("/workspace"),
            })
        }
    }

    #[tokio::test]
    async fn runtime_maps_completed_turn_to_agent_session() {
        let connector = MockConnector {
            inbound: vec![
                json_response(
                    "leaven-codex-1",
                    serde_json::json!({
                        "userAgent": "test",
                        "codexHome": "/tmp/codex-home",
                        "platformFamily": "unix",
                        "platformOs": "macos"
                    }),
                ),
                json_response("leaven-codex-2", thread_start_response(false)),
                json_response(
                    "leaven-codex-3",
                    serde_json::json!({
                        "turn": turn_payload("turn-1", "inProgress", [])
                    }),
                ),
                json_notification(ServerNotification::ItemCompleted(
                    ItemCompletedNotification {
                        thread_id: "thread-1".to_owned(),
                        turn_id: "turn-1".to_owned(),
                        item: serde_json::from_value(serde_json::json!({
                            "type": "agentMessage",
                            "id": "msg-1",
                            "text": "done"
                        }))
                        .unwrap(),
                        completed_at_ms: 0,
                    },
                )),
                json_notification(ServerNotification::TurnCompleted(
                    TurnCompletedNotification {
                        thread_id: "thread-1".to_owned(),
                        turn: serde_json::from_value(turn_payload(
                            "turn-1",
                            "completed",
                            [serde_json::json!({
                                "type": "agentMessage",
                                "id": "msg-1",
                                "text": "done"
                            })],
                        ))
                        .unwrap(),
                    },
                )),
            ],
            workspace_access: WorkspaceAccessMode::BackendNeutral,
        };
        let runtime = CodexAppServerRuntime::new(CodexAppServerConfig::default(), connector);
        let mut workspace = LocalWorkspaceFactory::temp()
            .allocate(Default::default())
            .await
            .unwrap();
        let mut view = workspace.view();
        let request = AgentRunRequest::new(
            AgentInstructions::task("say done"),
            OutputContract::FinalMessage,
        );
        let budget = BudgetSnapshot::default();

        let session = runtime
            .run_session(
                &mut view,
                request,
                AgentRunContext::new(AgentSessionId::new(), &budget),
            )
            .await
            .unwrap()
            .value;

        assert_eq!(session.status, AgentStatus::Succeeded);
        assert!(session.transcript.events.iter().any(|event| matches!(
            event,
            TranscriptEvent::Message {
                role: TranscriptRole::Assistant,
                content
            } if content == "done"
        )));
    }

    #[tokio::test]
    async fn ephemeral_runtime_does_not_refresh_not_loaded_turns() {
        let connector = MockConnector {
            inbound: vec![
                json_response(
                    "leaven-codex-1",
                    serde_json::json!({
                        "userAgent": "test",
                        "codexHome": "/tmp/codex-home",
                        "platformFamily": "unix",
                        "platformOs": "macos"
                    }),
                ),
                json_response("leaven-codex-2", thread_start_response(true)),
                json_response(
                    "leaven-codex-3",
                    serde_json::json!({
                        "turn": turn_payload("turn-1", "inProgress", [])
                    }),
                ),
                json_notification(ServerNotification::TurnCompleted(
                    TurnCompletedNotification {
                        thread_id: "thread-1".to_owned(),
                        turn: serde_json::from_value(turn_payload_not_loaded(
                            "turn-1",
                            "completed",
                        ))
                        .unwrap(),
                    },
                )),
            ],
            workspace_access: WorkspaceAccessMode::BackendNeutral,
        };
        let mut config = CodexAppServerConfig::default();
        config.thread.ephemeral = true;
        let runtime = CodexAppServerRuntime::new(config, connector);
        let mut workspace = LocalWorkspaceFactory::temp()
            .allocate(Default::default())
            .await
            .unwrap();
        let mut view = workspace.view();
        let request = AgentRunRequest::new(
            AgentInstructions::task("finish"),
            OutputContract::WorkspaceDiff { roots: Vec::new() },
        );
        let budget = BudgetSnapshot::default();

        let session = runtime
            .run_session(
                &mut view,
                request,
                AgentRunContext::new(AgentSessionId::new(), &budget),
            )
            .await
            .unwrap()
            .value;

        assert_eq!(session.status, AgentStatus::Succeeded);
    }

    #[tokio::test]
    async fn local_mount_requirement_is_reported_by_runtime_capability() {
        let connector = MockConnector {
            inbound: Vec::new(),
            workspace_access: WorkspaceAccessMode::RequiresLocalMount,
        };
        let runtime = CodexAppServerRuntime::new(CodexAppServerConfig::default(), connector);
        let mut workspace =
            leaven_workspace::Workspace::new(PathBuf::from("/remote"), Box::new(RemoteOnlyBackend));
        let mut view = workspace.view();
        let budget = BudgetSnapshot::default();

        let error = runtime
            .run_session(
                &mut view,
                AgentRunRequest::new(
                    AgentInstructions::task("x"),
                    OutputContract::WorkspaceDiff { roots: Vec::new() },
                ),
                AgentRunContext::new(AgentSessionId::new(), &budget),
            )
            .await
            .unwrap_err();

        assert!(matches!(
            error,
            AgentRuntimeError::LocalMountRequired { .. }
        ));
    }

    #[test]
    fn request_params_expose_model_effort_and_service_tier_controls() {
        let mut config = CodexAppServerConfig::default();
        config.thread.model = Some("gpt-5.4-mini".to_owned());
        config.thread.service_tier = Some("fast".to_owned());
        config.turn.model = Some("gpt-5.4-mini".to_owned());
        config.turn.service_tier = Some("fast".to_owned());
        config.turn.effort = Some(crate::CodexReasoningEffort::Low);

        let request = AgentRunRequest::new(
            AgentInstructions::task("write the file"),
            OutputContract::FinalMessage,
        );

        let thread = thread_start_params(&config, Path::new("/workspace"));
        let turn = turn_start_params(&config, "thread-1", Path::new("/workspace"), &request)
            .expect("turn params");

        assert_eq!(thread.model.as_deref(), Some("gpt-5.4-mini"));
        assert_eq!(thread.service_tier, Some(Some("fast".to_owned())));
        assert_eq!(turn.model.as_deref(), Some("gpt-5.4-mini"));
        assert_eq!(turn.service_tier, Some(Some("fast".to_owned())));
        assert!(matches!(
            turn.effort,
            Some(codex_protocol::openai_models::ReasoningEffort::Low)
        ));
    }

    fn json_response(id: &str, result: serde_json::Value) -> String {
        serde_json::to_string(&JSONRPCMessage::Response(JSONRPCResponse {
            id: RequestId::String(id.to_owned()),
            result,
        }))
        .unwrap()
    }

    fn json_notification(notification: ServerNotification) -> String {
        serde_json::to_string(&JSONRPCMessage::Notification(
            serde_json::from_value(serde_json::to_value(notification).unwrap()).unwrap(),
        ))
        .unwrap()
    }

    fn thread_start_response(ephemeral: bool) -> serde_json::Value {
        serde_json::to_value(ThreadStartResponse {
            thread: serde_json::from_value(serde_json::json!({
                "id": "thread-1",
                "sessionId": "session-1",
                "forkedFromId": null,
                "preview": "",
                "ephemeral": ephemeral,
                "modelProvider": "openai",
                "createdAt": 0,
                "updatedAt": 0,
                "status": {"type": "idle"},
                "path": null,
                "cwd": "/workspace",
                "cliVersion": "test",
                "source": "exec",
                "threadSource": null,
                "agentNickname": null,
                "agentRole": null,
                "gitInfo": null,
                "name": null,
                "turns": []
            }))
            .unwrap(),
            model: "gpt-5.4-mini".to_owned(),
            model_provider: "openai".to_owned(),
            service_tier: None,
            cwd: serde_json::from_value(serde_json::json!("/workspace")).unwrap(),
            instruction_sources: Vec::new(),
            approval_policy: serde_json::from_value(serde_json::json!("never")).unwrap(),
            approvals_reviewer: serde_json::from_value(serde_json::json!("user")).unwrap(),
            sandbox: serde_json::from_value(serde_json::json!({"type": "dangerFullAccess"}))
                .unwrap(),
            runtime_workspace_roots: Vec::new(),
            active_permission_profile: None,
            reasoning_effort: None,
        })
        .unwrap()
    }

    fn turn_payload<const N: usize>(
        id: &str,
        status: &str,
        items: [serde_json::Value; N],
    ) -> serde_json::Value {
        let items = items.into_iter().collect::<Vec<_>>();
        serde_json::json!({
            "id": id,
            "items": items,
            "itemsView": "full",
            "status": status,
            "error": null,
            "startedAt": null,
            "completedAt": null,
            "durationMs": null
        })
    }

    fn turn_payload_not_loaded(id: &str, status: &str) -> serde_json::Value {
        serde_json::json!({
            "id": id,
            "items": [],
            "itemsView": "notLoaded",
            "status": status,
            "error": null,
            "startedAt": null,
            "completedAt": null,
            "durationMs": null
        })
    }

    struct RemoteOnlyBackend;

    impl leaven_workspace::WorkspaceBackend for RemoteOnlyBackend {
        fn cleanup(
            self: Box<Self>,
        ) -> futures::future::BoxFuture<
            'static,
            std::result::Result<(), leaven_workspace::WorkspaceError>,
        > {
            Box::pin(async { Ok(()) })
        }
    }
}
