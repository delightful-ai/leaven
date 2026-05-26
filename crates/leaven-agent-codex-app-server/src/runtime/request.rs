use std::path::Path;

use codex_app_server_protocol::{ThreadStartParams, TurnStartParams, UserInput};
use leaven_agent::{AgentInstructions, AgentRunRequest, OutputContract};

use crate::config::CodexAppServerConfig;
use crate::error::{CodexAppServerError, Result as CodexResult};

pub(super) fn thread_start_params(config: &CodexAppServerConfig, cwd: &Path) -> ThreadStartParams {
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

pub(super) fn turn_start_params(
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
        OutputContract::JsonSchema { schema, .. } => Ok(Some(schema.clone())),
        OutputContract::Files { .. }
        | OutputContract::JsonFile { schema: None, .. }
        | OutputContract::FinalMessage
        | OutputContract::WorkspaceDiff { .. } => Ok(None),
    }
}
