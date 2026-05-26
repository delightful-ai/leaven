use serde_json::{Map, Value, json};

/// Errors raised while building agentic public-seam stage payloads.
#[derive(Debug, thiserror::Error)]
pub enum PublicStagePayloadError {
    /// A required stage-payload field was empty.
    #[error("public-seam stage payload `{field}` must not be empty")]
    EmptyField {
        /// Empty field name.
        field: &'static str,
    },
    /// Reflect and propose stages tried to reuse the same stage call.
    #[error("reflect and propose stages must use distinct stage_call_id values")]
    ReusedStageCall,
    /// The proposer was built from a different reflection result.
    #[error("propose request must consume the exact ReflectionResult in the handoff")]
    ReflectionMismatch,
    /// A runner payload tried to carry hidden target material.
    #[error("public-seam stage payload `{field}` must not carry case.target material")]
    TargetLeakage {
        /// Field containing hidden target material.
        field: &'static str,
    },
    /// A score or judge payload omitted assessed output data classes.
    #[error(
        "public-seam stage payload `{field}` must carry candidate output or artifact data class"
    )]
    MissingAssessedOutputClass {
        /// Output field missing assessed data classes.
        field: &'static str,
    },
    /// Scorer target access was not bound to the scored case.
    #[error("scorer target_handle must match the scored case")]
    TargetHandleMismatch,
    /// Stage payload fingerprinting failed.
    #[error("stage payload fingerprinting failed: {0}")]
    Fingerprint(String),
}

/// Shared public-seam identity fields for agentic stage payloads.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublicStagePayloadIdentity {
    pub(super) run: String,
    stage_call_id: String,
    pub(super) base_revision: String,
    pub(super) parent: Value,
    pub(super) source_refs: Vec<Value>,
    pub(super) surface_fingerprint: String,
    pub(super) query_policy_fingerprint: String,
    pub(super) capability_fingerprint: String,
}

/// Constructor fields for [`PublicStagePayloadIdentity`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PublicStagePayloadIdentityFields {
    /// Public run reference.
    pub run: String,
    /// Stage call id for this payload.
    pub stage_call_id: String,
    /// Base graph revision for the stage.
    pub base_revision: String,
    /// Parent candidate/source being reflected or proposed against.
    pub parent: Value,
    /// Source refs carried by the stage payload.
    pub source_refs: Vec<Value>,
    /// Fingerprint for the surfaced part being worked on.
    pub surface_fingerprint: String,
    /// Query policy fingerprint governing source reads.
    pub query_policy_fingerprint: String,
    /// Capability fingerprint authorizing this stage.
    pub capability_fingerprint: String,
}

impl PublicStagePayloadIdentity {
    /// Creates shared identity for one public-seam stage payload.
    pub fn new(fields: PublicStagePayloadIdentityFields) -> Result<Self, PublicStagePayloadError> {
        if fields.source_refs.is_empty() {
            return Err(PublicStagePayloadError::EmptyField {
                field: "source_refs",
            });
        }
        Ok(Self {
            run: super::util::non_empty(fields.run, "run")?,
            stage_call_id: super::util::non_empty(fields.stage_call_id, "stage_call_id")?,
            base_revision: super::util::non_empty(fields.base_revision, "base_revision")?,
            parent: fields.parent,
            source_refs: fields.source_refs,
            surface_fingerprint: super::util::non_empty(
                fields.surface_fingerprint,
                "surface_fingerprint",
            )?,
            query_policy_fingerprint: super::util::non_empty(
                fields.query_policy_fingerprint,
                "query_policy_fingerprint",
            )?,
            capability_fingerprint: super::util::non_empty(
                fields.capability_fingerprint,
                "capability_fingerprint",
            )?,
        })
    }

    pub(super) fn push_common(&self, object: &mut Map<String, Value>) {
        object.insert("run".to_owned(), json!(self.run));
        object.insert("stage_call_id".to_owned(), json!(self.stage_call_id));
        object.insert("base_revision".to_owned(), json!(self.base_revision));
        object.insert("parent".to_owned(), self.parent.clone());
        object.insert("source_refs".to_owned(), json!(self.source_refs));
        object.insert(
            "surface_fingerprint".to_owned(),
            json!(self.surface_fingerprint),
        );
        object.insert(
            "query_policy_fingerprint".to_owned(),
            json!(self.query_policy_fingerprint),
        );
        object.insert(
            "capability_fingerprint".to_owned(),
            json!(self.capability_fingerprint),
        );
    }

    pub(super) fn stage_call_id(&self) -> &str {
        &self.stage_call_id
    }
}
