use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;

use crate::PlanEmitRunEventWrite;

use super::workspace_ref_object;

/// Host outcome for a typed `workspace_materialize` call.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanWorkspaceMaterializeOutcome {
    pub(in crate::plan_execution) workspace: String,
    pub(in crate::plan_execution) workspace_ref: Value,
    pub(in crate::plan_execution) lifetime: String,
    pub(in crate::plan_execution) data_classes: Vec<String>,
    pub(in crate::plan_execution) replayability: String,
    pub(in crate::plan_execution) runtime_fingerprint: String,
}

impl PlanWorkspaceMaterializeOutcome {
    /// Creates a live workspace handle outcome.
    #[must_use]
    pub fn new(
        workspace: impl Into<String>,
        lifetime: impl Into<String>,
        runtime_fingerprint: impl Into<String>,
    ) -> Self {
        let workspace = workspace.into();
        Self {
            workspace_ref: Value::String(workspace.clone()),
            workspace,
            lifetime: lifetime.into(),
            data_classes: vec!["public".to_owned()],
            replayability: "boundary_managed".to_owned(),
            runtime_fingerprint: runtime_fingerprint.into(),
        }
    }

    #[must_use]
    pub fn with_workspace_object_ref(
        mut self,
        run: Option<impl Into<String>>,
        snapshot_fingerprint: Option<impl Into<String>>,
    ) -> Self {
        self.workspace_ref = workspace_ref_object(
            &self.workspace,
            run.map(Into::into),
            snapshot_fingerprint.map(Into::into),
        );
        self
    }
}

/// Host outcome for a typed `workspace_release` call.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanWorkspaceReleaseOutcome {
    pub(in crate::plan_execution) workspace: String,
    pub(in crate::plan_execution) workspace_ref: Value,
    pub(in crate::plan_execution) lifetime: String,
    pub(in crate::plan_execution) runtime_fingerprint: String,
}

impl PlanWorkspaceReleaseOutcome {
    /// Creates a workspace release outcome.
    #[must_use]
    pub fn new(
        workspace: impl Into<String>,
        lifetime: impl Into<String>,
        runtime_fingerprint: impl Into<String>,
    ) -> Self {
        let workspace = workspace.into();
        Self {
            workspace_ref: Value::String(workspace.clone()),
            workspace,
            lifetime: lifetime.into(),
            runtime_fingerprint: runtime_fingerprint.into(),
        }
    }

    #[must_use]
    pub fn with_workspace_object_ref(
        mut self,
        run: Option<impl Into<String>>,
        snapshot_fingerprint: Option<impl Into<String>>,
    ) -> Self {
        self.workspace_ref = workspace_ref_object(
            &self.workspace,
            run.map(Into::into),
            snapshot_fingerprint.map(Into::into),
        );
        self
    }
}

/// Lowered `emit_run_event` request passed to a plan execution host.
#[derive(Clone, Copy, Debug)]
pub struct PlanEmitRunEventRequest<'a> {
    pub(in crate::plan_execution) name: &'a str,
    pub(in crate::plan_execution) write: &'a PlanEmitRunEventWrite,
    pub(in crate::plan_execution) deps: &'a BTreeMap<String, Value>,
    pub(in crate::plan_execution) dependency_data_classes: &'a BTreeSet<String>,
    pub(in crate::plan_execution) base_revision: &'a str,
}

impl<'a> PlanEmitRunEventRequest<'a> {
    /// Operation binding name.
    pub const fn name(&self) -> &'a str {
        self.name
    }

    /// Typed `emit_run_event` write body from the Plan IR.
    pub const fn write(&self) -> &'a PlanEmitRunEventWrite {
        self.write
    }

    /// Resolved dependency bindings visible to this write.
    pub const fn deps(&self) -> &'a BTreeMap<String, Value> {
        self.deps
    }

    /// Data classes carried by dependency bindings but not necessarily present
    /// in the host-visible JSON values.
    pub const fn dependency_data_classes(&self) -> &'a BTreeSet<String> {
        self.dependency_data_classes
    }

    /// Base graph revision supplied by the public-seam execution context.
    pub const fn base_revision(&self) -> &'a str {
        self.base_revision
    }
}

/// Host outcome for a typed `emit_run_event` write.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanEmitRunEventOutcome {
    pub(in crate::plan_execution) event_id: String,
    pub(in crate::plan_execution) committed_revision: String,
}

impl PlanEmitRunEventOutcome {
    /// Creates an emitted event outcome.
    pub fn new(event_id: impl Into<String>, committed_revision: impl Into<String>) -> Self {
        Self {
            event_id: event_id.into(),
            committed_revision: committed_revision.into(),
        }
    }
}
