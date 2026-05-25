/// Host outcome for a typed `workspace_materialize` call.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanWorkspaceMaterializeOutcome {
    pub(super) workspace: String,
    pub(super) workspace_ref: Value,
    pub(super) lifetime: String,
    pub(super) data_classes: Vec<String>,
    pub(super) replayability: String,
    pub(super) runtime_fingerprint: String,
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
    pub(super) workspace: String,
    pub(super) workspace_ref: Value,
    pub(super) lifetime: String,
    pub(super) runtime_fingerprint: String,
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
    pub(super) name: &'a str,
    pub(super) write: &'a Value,
    pub(super) deps: &'a BTreeMap<String, Value>,
    pub(super) dependency_data_classes: &'a BTreeSet<String>,
    pub(super) base_revision: &'a str,
}

impl<'a> PlanEmitRunEventRequest<'a> {
    /// Operation binding name.
    pub const fn name(&self) -> &'a str {
        self.name
    }

    /// Typed `emit_run_event` write body from the Plan IR.
    pub const fn write(&self) -> &'a Value {
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
    pub(super) event_id: String,
    pub(super) committed_revision: String,
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
