use serde_json::{Value, json};

/// The locked V1 extension-method profile rows, in canonical order.
///
/// Each row carries the locked `params_schema`/`result_schema` binding, the
/// `required_action` capability path, and `produces_receipt`, exactly as the
/// profile validator demands. This is the single source the canonical locked
/// profile document is assembled from, so the engine client, the bridge, and the
/// conformance tests stop re-encoding the 26-method table by hand.
pub(super) fn locked_extension_method_rows() -> Vec<Value> {
    locked_extension_methods()
        .into_iter()
        .map(|method| {
            let (params, result) =
                schema_binding_for_method(method).expect("locked method has a schema binding");
            let action =
                required_action_for_method(method).expect("locked method has a required action");
            json!({
                "method": method,
                "params_schema": params.schema_file(),
                "result_schema": result.schema_file(),
                "required_action": action,
                "produces_receipt": true
            })
        })
        .collect()
}

/// Schema bound to a Leaven ACP extension method's params or result.
///
/// The 25 worker callbacks bind the Plan IR schemas, while the one host->worker
/// stage-dispatch method binds the dedicated stage-run schema.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum MethodSchema {
    /// Locked Plan IR request schema (`leaven.plan.v1.schema.json`).
    PlanIr,
    /// Locked Plan Result schema (`leaven.plan_result.v1.schema.json`).
    PlanResult,
    /// Dedicated stage-run schema (`leaven.stage_run.v1.schema.json`).
    StageRun,
}

impl MethodSchema {
    pub(super) const fn schema_file(self) -> &'static str {
        match self {
            Self::PlanIr => "leaven.plan.v1.schema.json",
            Self::PlanResult => "leaven.plan_result.v1.schema.json",
            Self::StageRun => "leaven.stage_run.v1.schema.json",
        }
    }
}

pub(super) fn locked_extension_methods() -> [&'static str; 26] {
    [
        "leaven/stage.run",
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
        "leaven/stage.run" => Some("stage.run"),
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

/// Locked params/result schema bindings for a Leaven ACP extension method.
///
/// `leaven/stage.run` is host->worker stage dispatch and binds the dedicated
/// stage-run schema in both directions; every other locked method is a
/// worker->host Plan IR effect callback and binds Plan IR params plus a Plan
/// Result.
pub(super) fn schema_binding_for_method(method: &str) -> Option<(MethodSchema, MethodSchema)> {
    // Only methods in the locked set carry a binding; the stage-dispatch method
    // binds the stage-run schema in both directions and every effect callback
    // binds Plan IR params plus a Plan Result.
    required_action_for_method(method)?;
    Some(match method {
        "leaven/stage.run" => (MethodSchema::StageRun, MethodSchema::StageRun),
        _ => (MethodSchema::PlanIr, MethodSchema::PlanResult),
    })
}
