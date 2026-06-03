use std::collections::BTreeMap;

use leaven_workspace::WorkspacePath;
use serde_json::{Value, json};

use crate::PublicSeamError;

mod agent;
mod blob_ref;
mod lm;
mod outcomes;
mod proposal;
mod sandbox;

pub use agent::{AgentCommandOutputRefs, PlanAgentRunOutcome, PlanAgentRunRequest};
pub use lm::{PlanLmCompleteOutcome, PlanLmCompleteRequest};
pub use outcomes::{
    PlanEmitRunEventOutcome, PlanEmitRunEventRequest, PlanWorkspaceMaterializeOutcome,
    PlanWorkspaceReleaseOutcome,
};
pub use proposal::{PlanSubmitProposalBatchOutcome, PlanSubmitProposalBatchRequest};
pub use sandbox::{PlanSandboxExecOutcome, PlanSandboxExecRequest};

/// Lowered `workspace_materialize` request passed to a plan execution host.
#[derive(Clone, Copy, Debug)]
pub struct PlanWorkspaceMaterializeRequest<'a> {
    pub(super) name: &'a str,
    pub(super) call: &'a Value,
    pub(super) deps: &'a BTreeMap<String, Value>,
}

impl<'a> PlanWorkspaceMaterializeRequest<'a> {
    /// Operation binding name.
    pub const fn name(&self) -> &'a str {
        self.name
    }

    /// Typed `workspace_materialize` call body from the Plan IR.
    pub const fn call(&self) -> &'a Value {
        self.call
    }

    /// Resolved dependency bindings visible to this call.
    pub const fn deps(&self) -> &'a BTreeMap<String, Value> {
        self.deps
    }

    /// Candidate being materialized.
    pub fn candidate(&self) -> Result<&'a str, PublicSeamError> {
        self.call
            .get("candidate")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid_call("workspace_materialize must carry candidate"))
    }

    /// Optional surface selector.
    #[must_use]
    pub fn surface(&self) -> Option<&'a str> {
        self.call.get("surface").and_then(Value::as_str)
    }

    /// Workspace materialization mode.
    pub fn mode(&self) -> Result<&'a str, PublicSeamError> {
        self.call
            .get("mode")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid_call("workspace_materialize must carry mode"))
    }

    /// Requested workspace lifetime.
    pub fn lifetime(&self) -> Result<&'a str, PublicSeamError> {
        self.call
            .get("lifetime")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid_call("workspace_materialize must carry lifetime"))
    }
}

/// Lowered `workspace_release` request passed to a plan execution host.
#[derive(Clone, Copy, Debug)]
pub struct PlanWorkspaceReleaseRequest<'a> {
    pub(super) name: &'a str,
    pub(super) call: &'a Value,
    pub(super) deps: &'a BTreeMap<String, Value>,
    pub(super) live_workspaces: &'a BTreeMap<String, LiveWorkspaceHandle>,
}

impl<'a> PlanWorkspaceReleaseRequest<'a> {
    /// Operation binding name.
    pub const fn name(&self) -> &'a str {
        self.name
    }

    /// Typed `workspace_release` call body from the Plan IR.
    pub const fn call(&self) -> &'a Value {
        self.call
    }

    /// Resolved dependency bindings visible to this call.
    pub const fn deps(&self) -> &'a BTreeMap<String, Value> {
        self.deps
    }

    /// Workspace handle requested for release.
    pub fn workspace(&self) -> Result<&'a str, PublicSeamError> {
        workspace_ref_id(
            self.call.get("workspace"),
            "workspace_release must carry workspace",
        )
    }

    pub(super) fn workspace_ref(&self) -> Result<WorkspaceRefFacts, PublicSeamError> {
        workspace_ref_facts(
            self.call.get("workspace"),
            "workspace_release must carry workspace",
        )
    }

    /// Workspace handle requested for release, proven against live dependency handles.
    pub fn live_workspace(&self) -> Result<&'a str, PublicSeamError> {
        let workspace = self.workspace_ref()?;
        Ok(require_live_workspace_ref(
            &workspace,
            self.deps,
            self.live_workspaces,
            "workspace_release",
        )?
        .workspace())
    }

    /// Lifetime attached to the live dependency handle being released.
    pub(super) fn live_workspace_lifetime(&self) -> Result<&'a str, PublicSeamError> {
        let workspace = self.workspace_ref()?;
        Ok(require_live_workspace_ref(
            &workspace,
            self.deps,
            self.live_workspaces,
            "workspace_release",
        )?
        .lifetime())
    }

    /// Whether release may force cleanup.
    #[must_use]
    pub fn force(&self) -> bool {
        self.call
            .get("force")
            .and_then(Value::as_bool)
            .unwrap_or(false)
    }
}

pub(super) fn required_object<'a>(
    value: &'a Value,
    key: &str,
    message: impl Into<String>,
) -> Result<&'a serde_json::Map<String, Value>, PublicSeamError> {
    value
        .get(key)
        .and_then(Value::as_object)
        .ok_or_else(|| invalid_call(message))
}

pub(super) fn workspace_path(path: &str, context: &str) -> Result<WorkspacePath, PublicSeamError> {
    if path == "." {
        return Ok(WorkspacePath::root());
    }
    WorkspacePath::new(path).map_err(|error| invalid_call(format!("{context}: {error}")))
}

pub(super) fn workspace_ref_id(
    value: Option<&Value>,
    context: impl Into<String>,
) -> Result<&str, PublicSeamError> {
    let context = context.into();
    let value = value.ok_or_else(|| invalid_call(context.clone()))?;
    if let Some(workspace) = value.as_str() {
        return Ok(workspace);
    }
    let object = value.as_object().ok_or_else(|| {
        invalid_call(format!("{context}: workspace ref must be string or object"))
    })?;
    if object.get("kind").and_then(Value::as_str) != Some("workspace") {
        return Err(invalid_call(format!(
            "{context}: workspace ref object must have kind `workspace`"
        )));
    }
    object
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid_call(format!("{context}: workspace ref object must carry id")))
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct WorkspaceRefFacts {
    id: String,
    run: Option<String>,
    snapshot_fingerprint: Option<String>,
}

impl WorkspaceRefFacts {
    fn from_id(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            run: None,
            snapshot_fingerprint: None,
        }
    }

    fn from_value(
        value: Option<&Value>,
        context: impl Into<String>,
    ) -> Result<Self, PublicSeamError> {
        let context = context.into();
        let value = value.ok_or_else(|| invalid_call(context.clone()))?;
        if let Some(workspace) = value.as_str() {
            return Ok(Self::from_id(workspace));
        }
        let object = value.as_object().ok_or_else(|| {
            invalid_call(format!("{context}: workspace ref must be string or object"))
        })?;
        if object.get("kind").and_then(Value::as_str) != Some("workspace") {
            return Err(invalid_call(format!(
                "{context}: workspace ref object must have kind `workspace`"
            )));
        }
        let id = object
            .get("id")
            .and_then(Value::as_str)
            .ok_or_else(|| invalid_call(format!("{context}: workspace ref object must carry id")))?
            .to_owned();
        let run = optional_string(object.get("run"), "workspace ref run")?;
        let snapshot_fingerprint = optional_string(
            object.get("snapshot_fingerprint"),
            "workspace ref snapshot_fingerprint",
        )?;
        Ok(Self {
            id,
            run,
            snapshot_fingerprint,
        })
    }

    pub(super) fn id(&self) -> &str {
        &self.id
    }

    pub(super) fn to_value(&self) -> Value {
        if self.run.is_none() && self.snapshot_fingerprint.is_none() {
            return Value::String(self.id.clone());
        }
        let mut value = json!({
            "kind": "workspace",
            "id": self.id
        });
        if let Some(run) = &self.run {
            value["run"] = json!(run);
        }
        if let Some(snapshot_fingerprint) = &self.snapshot_fingerprint {
            value["snapshot_fingerprint"] = json!(snapshot_fingerprint);
        }
        value
    }

    pub(super) fn satisfies_request(&self, requested: &Self) -> bool {
        self.id == requested.id
            && self.run == requested.run
            && self.snapshot_fingerprint == requested.snapshot_fingerprint
    }
}

pub(super) fn workspace_ref_facts(
    value: Option<&Value>,
    context: impl Into<String>,
) -> Result<WorkspaceRefFacts, PublicSeamError> {
    WorkspaceRefFacts::from_value(value, context)
}

fn workspace_ref_object(
    workspace: &str,
    run: Option<String>,
    snapshot_fingerprint: Option<String>,
) -> Value {
    WorkspaceRefFacts {
        id: workspace.to_owned(),
        run,
        snapshot_fingerprint,
    }
    .to_value()
}

fn optional_string(value: Option<&Value>, field: &str) -> Result<Option<String>, PublicSeamError> {
    value
        .map(|value| {
            value
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| invalid_call(format!("{field} must be a string")))
        })
        .transpose()
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct LiveWorkspaceHandle {
    workspace: WorkspaceRefFacts,
    lifetime: String,
    released: bool,
}

impl LiveWorkspaceHandle {
    pub(super) fn live_ref(workspace: WorkspaceRefFacts, lifetime: impl Into<String>) -> Self {
        Self {
            workspace,
            lifetime: lifetime.into(),
            released: false,
        }
    }

    pub(super) fn released_ref(workspace: WorkspaceRefFacts, lifetime: impl Into<String>) -> Self {
        Self {
            workspace,
            lifetime: lifetime.into(),
            released: true,
        }
    }

    pub(super) fn release(&mut self) {
        self.released = true;
    }

    pub(super) fn satisfies_workspace(&self, requested: &WorkspaceRefFacts) -> bool {
        self.workspace.satisfies_request(requested)
    }

    pub(super) fn workspace(&self) -> &str {
        self.workspace.id()
    }

    pub(super) fn lifetime(&self) -> &str {
        &self.lifetime
    }
}

pub(super) fn require_live_workspace_ref<'a>(
    requested: &WorkspaceRefFacts,
    deps: &'a BTreeMap<String, Value>,
    live_workspaces: &'a BTreeMap<String, LiveWorkspaceHandle>,
    context: &str,
) -> Result<&'a LiveWorkspaceHandle, PublicSeamError> {
    let Some((dep_name, handle_value)) = deps.iter().find(|(_, value)| {
        value.get("kind").and_then(Value::as_str) == Some("workspace_handle")
            && value
                .get("workspace")
                .and_then(|value| workspace_ref_facts(Some(value), "workspace handle").ok())
                .is_some_and(|available| available.satisfies_request(requested))
    }) else {
        return Err(invalid_call(format!(
            "{context} refused unmaterialized workspace `{}`",
            requested.id()
        )));
    };
    let Some(handle) = live_workspaces.get(dep_name) else {
        return Err(invalid_call(format!(
            "{context} refused unmaterialized workspace `{}`",
            requested.id()
        )));
    };
    if !handle.workspace.satisfies_request(requested) {
        return Err(invalid_call(format!(
            "{context} refused unmaterialized workspace `{}`",
            requested.id()
        )));
    }
    if handle.released
        || handle_value
            .get("released")
            .and_then(Value::as_bool)
            .unwrap_or(false)
    {
        return Err(invalid_call(format!(
            "{context} refused already released workspace `{}`",
            requested.id()
        )));
    }
    Ok(handle)
}

pub(super) fn invalid_call(message: impl Into<String>) -> PublicSeamError {
    PublicSeamError::InvalidPlan {
        message: message.into(),
    }
}

fn cost_value(cost: &leaven_kernel::Cost) -> Value {
    let mut object = serde_json::Map::new();
    if cost.prompt_tokens > 0 {
        object.insert("input_tokens".to_owned(), json!(cost.prompt_tokens));
    }
    if cost.completion_tokens > 0 {
        object.insert("output_tokens".to_owned(), json!(cost.completion_tokens));
    }
    if cost.llm_calls > 0 {
        object.insert("lm_calls".to_owned(), json!(cost.llm_calls));
    }
    if cost.metric_calls > 0 {
        object.insert("metric_calls".to_owned(), json!(cost.metric_calls));
    }
    insert_count_cost_axis(&mut object, cost, "agent_calls");
    insert_count_cost_axis(&mut object, cost, "sandbox_calls");
    insert_count_cost_axis(&mut object, cost, "usd_micro");
    insert_count_cost_axis(&mut object, cost, "human_review_usd_micro");
    insert_count_cost_axis(&mut object, cost, "wall_ms");
    Value::Object(object)
}

fn insert_count_cost_axis(
    object: &mut serde_json::Map<String, Value>,
    cost: &leaven_kernel::Cost,
    axis: &str,
) {
    let Some(amount) = cost.other.get(axis) else {
        return;
    };
    let amount = amount.as_f64();
    if amount > 0.0
        && amount.fract() == 0.0
        && let Ok(amount) = amount.to_string().parse::<u64>()
    {
        object.insert(axis.to_owned(), json!(amount));
    }
}

fn extend_data_classes_from_blob_ref(data_classes: &mut Vec<String>, blob_ref: &Value) {
    let Some(blob_data_classes) = blob_ref
        .as_object()
        .and_then(|object| object.get("data_classes"))
        .and_then(Value::as_array)
    else {
        return;
    };
    for data_class in blob_data_classes.iter().filter_map(Value::as_str) {
        blob_ref::push_unique_data_class(data_classes, data_class);
    }
}
