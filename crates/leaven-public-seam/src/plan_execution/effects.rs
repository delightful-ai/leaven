use std::collections::BTreeMap;

use serde_json::{Value, json};

/// Lowered `lm_complete` request passed to a plan execution host.
#[derive(Clone, Copy, Debug)]
pub struct PlanLmCompleteRequest<'a> {
    pub(super) name: &'a str,
    pub(super) call: &'a Value,
    pub(super) deps: &'a BTreeMap<String, Value>,
}

impl<'a> PlanLmCompleteRequest<'a> {
    /// Operation binding name.
    pub const fn name(&self) -> &'a str {
        self.name
    }

    /// Typed `lm_complete` call body from the Plan IR.
    pub const fn call(&self) -> &'a Value {
        self.call
    }

    /// Resolved dependency bindings visible to this call.
    pub const fn deps(&self) -> &'a BTreeMap<String, Value> {
        self.deps
    }
}

/// Host outcome for a typed `lm_complete` call.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanLmCompleteOutcome {
    pub(super) message: Value,
    pub(super) data_classes: Vec<String>,
    pub(super) replayability: String,
    pub(super) runtime_fingerprint: String,
    pub(super) error: Option<Value>,
    pub(super) cost: Option<Value>,
}

impl PlanLmCompleteOutcome {
    /// Creates an LM response outcome.
    pub fn new(message: Value, runtime_fingerprint: impl Into<String>) -> Self {
        Self {
            message,
            data_classes: vec!["public".to_owned()],
            replayability: "fully_managed".to_owned(),
            runtime_fingerprint: runtime_fingerprint.into(),
            error: None,
            cost: None,
        }
    }

    /// Creates a failed paid LM outcome that still emits audit and charge receipts.
    pub fn failed_provider_error(
        message: impl Into<String>,
        runtime_fingerprint: impl Into<String>,
        usd_micro: u64,
    ) -> Self {
        Self {
            message: Value::Null,
            data_classes: Vec::new(),
            replayability: "has_declared_external_effects".to_owned(),
            runtime_fingerprint: runtime_fingerprint.into(),
            error: Some(json!({
                "code": "provider_error",
                "message": message.into(),
                "retryable": true
            })),
            cost: Some(json!({
                "usd_micro": usd_micro
            })),
        }
    }

    /// Overrides the data classes carried by the LM response value.
    #[must_use]
    pub fn with_data_classes(mut self, data_classes: impl IntoIterator<Item = String>) -> Self {
        self.data_classes = data_classes.into_iter().collect();
        self
    }

    /// Overrides the replayability classification carried by the LM response value.
    #[must_use]
    pub fn with_replayability(mut self, replayability: impl Into<String>) -> Self {
        self.replayability = replayability.into();
        self
    }
}

/// Lowered `emit_run_event` request passed to a plan execution host.
#[derive(Clone, Copy, Debug)]
pub struct PlanEmitRunEventRequest<'a> {
    pub(super) name: &'a str,
    pub(super) write: &'a Value,
    pub(super) deps: &'a BTreeMap<String, Value>,
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
