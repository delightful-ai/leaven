use serde_json::{Value, json};

pub(super) fn agent_status_value(status: &leaven_agent::AgentStatus) -> &'static str {
    match status {
        leaven_agent::AgentStatus::Succeeded => "completed",
        leaven_agent::AgentStatus::Failed { .. }
        | leaven_agent::AgentStatus::OutputContractViolation { .. } => "failed",
        leaven_agent::AgentStatus::Cancelled => "cancelled",
        leaven_agent::AgentStatus::TimedOut => "timeout",
    }
}

pub(super) fn agent_command_value(command: &leaven_agent::CommandRecord, receipt: &str) -> Value {
    let mut argv = Vec::with_capacity(command.command.args.len() + 1);
    argv.push(command.command.program.clone());
    argv.extend(command.command.args.clone());
    json!({
        "argv": argv,
        "status": command_status_value(command.output.status),
        "receipt": receipt
    })
}

fn command_status_value(status: leaven_workspace::ExitStatus) -> &'static str {
    if status.code == Some(0) {
        "completed"
    } else {
        "failed"
    }
}
