pub(super) fn locked_extension_methods() -> [&'static str; 25] {
    [
        "leaven/graph.query",
        "leaven/case.load",
        "leaven/case.input",
        "leaven/case.target",
        "leaven/case.metadata",
        "leaven/workspace.materialize",
        "leaven/workspace.snapshot",
        "leaven/workspace.list",
        "leaven/workspace.read_file",
        "leaven/workspace.stat",
        "leaven/workspace.digest",
        "leaven/workspace.git_log",
        "leaven/workspace.git_diff",
        "leaven/workspace.git_status",
        "leaven/workspace.capture_artifacts",
        "leaven/workspace.release",
        "leaven/lm.complete",
        "leaven/agent.run",
        "leaven/sandbox.exec",
        "leaven/human.review",
        "leaven/proposal.submit_batch",
        "leaven/proposal.apply",
        "leaven/assessment.submit",
        "leaven/evaluation.request",
        "leaven/event.emit",
    ]
}

pub(super) fn required_action_for_method(method: &str) -> Option<&'static str> {
    match method {
        "leaven/graph.query" => Some("graph.query"),
        "leaven/case.load"
        | "leaven/case.input"
        | "leaven/case.target"
        | "leaven/case.metadata" => Some("case.read"),
        "leaven/workspace.materialize" => Some("workspace.materialize"),
        "leaven/workspace.snapshot"
        | "leaven/workspace.list"
        | "leaven/workspace.read_file"
        | "leaven/workspace.stat"
        | "leaven/workspace.digest"
        | "leaven/workspace.git_log"
        | "leaven/workspace.git_diff"
        | "leaven/workspace.git_status"
        | "leaven/workspace.capture_artifacts" => Some("workspace.read"),
        "leaven/workspace.release" => Some("workspace.release"),
        "leaven/lm.complete" => Some("lm.complete"),
        "leaven/agent.run" => Some("agent.run"),
        "leaven/sandbox.exec" => Some("sandbox.exec"),
        "leaven/human.review" => Some("human.review"),
        "leaven/proposal.submit_batch" => Some("proposal.submit_batch"),
        "leaven/proposal.apply" => Some("proposal.apply_batch"),
        "leaven/assessment.submit" => Some("assessment.submit"),
        "leaven/evaluation.request" => Some("evaluation.request"),
        "leaven/event.emit" => Some("event.emit"),
        _ => None,
    }
}
