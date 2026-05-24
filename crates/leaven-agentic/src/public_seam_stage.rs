//! Agentic lowering helpers for the public-seam reflection/proposal split.

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
    /// Stage payload fingerprinting failed.
    #[error("stage payload fingerprinting failed: {0}")]
    Fingerprint(String),
}

/// Shared public-seam identity fields for agentic stage payloads.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PublicStagePayloadIdentity {
    run: String,
    stage_call_id: String,
    base_revision: String,
    parent: Value,
    source_refs: Vec<Value>,
    surface_fingerprint: String,
    query_policy_fingerprint: String,
    capability_fingerprint: String,
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
            run: non_empty(fields.run, "run")?,
            stage_call_id: non_empty(fields.stage_call_id, "stage_call_id")?,
            base_revision: non_empty(fields.base_revision, "base_revision")?,
            parent: fields.parent,
            source_refs: fields.source_refs,
            surface_fingerprint: non_empty(fields.surface_fingerprint, "surface_fingerprint")?,
            query_policy_fingerprint: non_empty(
                fields.query_policy_fingerprint,
                "query_policy_fingerprint",
            )?,
            capability_fingerprint: non_empty(
                fields.capability_fingerprint,
                "capability_fingerprint",
            )?,
        })
    }

    fn push_common(&self, object: &mut Map<String, Value>) {
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

    fn stage_call_id(&self) -> &str {
        &self.stage_call_id
    }
}

/// Agentic `ReflectRequest` payload ready for public-seam validation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReflectRequestPayload {
    identity: PublicStagePayloadIdentity,
    value: Value,
}

impl ReflectRequestPayload {
    /// Builds a target-safe `ReflectRequest` payload.
    pub fn new(
        identity: PublicStagePayloadIdentity,
        part_label: impl Into<String>,
        examples: impl IntoIterator<Item = Value>,
    ) -> Result<Self, PublicStagePayloadError> {
        let examples = examples.into_iter().collect::<Vec<_>>();
        if examples.is_empty() {
            return Err(PublicStagePayloadError::EmptyField { field: "examples" });
        }
        let part_label = non_empty(part_label.into(), "part_label")?;
        let mut object = Map::new();
        object.insert(
            "schema_version".to_owned(),
            json!("leaven.stage_payloads.v1"),
        );
        object.insert("role".to_owned(), json!("reflector"));
        identity.push_common(&mut object);
        object.insert("part_label".to_owned(), json!(part_label));
        object.insert("examples".to_owned(), json!(examples));
        object.insert("target_safety".to_owned(), json!("target_safe_projection"));
        Ok(Self {
            identity,
            value: Value::Object(object),
        })
    }

    /// Returns the public-seam wire payload.
    #[must_use]
    pub const fn value(&self) -> &Value {
        &self.value
    }
}

/// Agentic `ReflectionResult` payload ready for public-seam validation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReflectionResultPayload {
    value: Value,
    fingerprint: String,
}

impl ReflectionResultPayload {
    /// Builds a receipted `ReflectionResult` payload.
    pub fn new(
        summary: impl Into<String>,
        failure_modes: impl IntoIterator<Item = Value>,
        surface_suggestions: impl IntoIterator<Item = Value>,
        source_refs: impl IntoIterator<Item = Value>,
        read_receipts: impl IntoIterator<Item = Value>,
        data_classes: impl IntoIterator<Item = String>,
        confidence: f64,
    ) -> Result<Self, PublicStagePayloadError> {
        let failure_modes = failure_modes.into_iter().collect::<Vec<_>>();
        let surface_suggestions = surface_suggestions.into_iter().collect::<Vec<_>>();
        if failure_modes.is_empty() && surface_suggestions.is_empty() {
            return Err(PublicStagePayloadError::EmptyField { field: "diagnosis" });
        }
        let source_refs = source_refs.into_iter().collect::<Vec<_>>();
        if source_refs.is_empty() {
            return Err(PublicStagePayloadError::EmptyField {
                field: "source_refs",
            });
        }
        let read_receipts = read_receipts.into_iter().collect::<Vec<_>>();
        if read_receipts.is_empty() {
            return Err(PublicStagePayloadError::EmptyField {
                field: "read_receipts",
            });
        }
        let data_classes = data_classes.into_iter().collect::<Vec<_>>();
        if data_classes.is_empty() {
            return Err(PublicStagePayloadError::EmptyField {
                field: "data_classes",
            });
        }
        let value = json!({
            "schema_version": "leaven.stage_payloads.v1",
            "role": "reflection_result",
            "summary": non_empty(summary.into(), "summary")?,
            "failure_modes": failure_modes,
            "surface_suggestions": surface_suggestions,
            "source_refs": source_refs,
            "read_receipts": read_receipts,
            "data_classes": data_classes,
            "confidence": confidence
        });
        let fingerprint = stage_payload_fingerprint(&value)?;
        Ok(Self { value, fingerprint })
    }

    /// Returns the public-seam wire payload.
    #[must_use]
    pub const fn value(&self) -> &Value {
        &self.value
    }

    /// Returns the `fp_stage_payload_sha256_*` fingerprint for this result.
    #[must_use]
    pub fn fingerprint(&self) -> &str {
        &self.fingerprint
    }
}

/// Agentic `ProposeRequest` payload that consumes a typed `ReflectionResult`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProposeRequestPayload {
    identity: PublicStagePayloadIdentity,
    reflection: ReflectionResultPayload,
    value: Value,
}

impl ProposeRequestPayload {
    /// Builds a `ProposeRequest` payload from an already-produced `ReflectionResult`.
    pub fn from_reflection(
        identity: PublicStagePayloadIdentity,
        reflection: ReflectionResultPayload,
        allowed_effects: impl IntoIterator<Item = String>,
        allowed_change_schemas: impl IntoIterator<Item = String>,
    ) -> Result<Self, PublicStagePayloadError> {
        let allowed_effects = allowed_effects.into_iter().collect::<Vec<_>>();
        if allowed_effects.is_empty() {
            return Err(PublicStagePayloadError::EmptyField {
                field: "allowed_effects",
            });
        }
        let allowed_change_schemas = allowed_change_schemas.into_iter().collect::<Vec<_>>();
        let mut object = Map::new();
        object.insert(
            "schema_version".to_owned(),
            json!("leaven.stage_payloads.v1"),
        );
        object.insert("role".to_owned(), json!("proposer"));
        identity.push_common(&mut object);
        object.insert("reflection_result".to_owned(), reflection.value.clone());
        object.insert("allowed_effects".to_owned(), json!(allowed_effects));
        object.insert(
            "allowed_change_schemas".to_owned(),
            json!(allowed_change_schemas),
        );
        Ok(Self {
            identity,
            reflection,
            value: Value::Object(object),
        })
    }

    /// Returns the public-seam wire payload.
    #[must_use]
    pub const fn value(&self) -> &Value {
        &self.value
    }
}

/// Complete reflect/propose handoff payload with binding stage receipts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReflectProposeHandoffPayload {
    value: Value,
}

impl ReflectProposeHandoffPayload {
    /// Builds a public-seam reflect/propose handoff from distinct stage outputs.
    pub fn new(
        reflect: &ReflectRequestPayload,
        reflection: &ReflectionResultPayload,
        propose: &ProposeRequestPayload,
        reflect_stage_receipt: impl Into<String>,
        propose_stage_receipt: impl Into<String>,
    ) -> Result<Self, PublicStagePayloadError> {
        if reflect.identity.stage_call_id() == propose.identity.stage_call_id() {
            return Err(PublicStagePayloadError::ReusedStageCall);
        }
        if propose.reflection.fingerprint() != reflection.fingerprint() {
            return Err(PublicStagePayloadError::ReflectionMismatch);
        }
        let reflect_stage_receipt =
            non_empty(reflect_stage_receipt.into(), "reflect_stage_receipt")?;
        let propose_stage_receipt =
            non_empty(propose_stage_receipt.into(), "propose_stage_receipt")?;
        Ok(Self {
            value: json!({
                "reflect_request": reflect.value,
                "reflection_result": reflection.value,
                "propose_request": propose.value,
                "stage_receipts": [
                    {
                        "kind": "stage_receipt",
                        "id": reflect_stage_receipt,
                        "stage_call_id": reflect.identity.stage_call_id(),
                        "stage_role": "reflector",
                        "produces": {
                            "kind": "reflection_result",
                            "fingerprint": reflection.fingerprint()
                        }
                    },
                    {
                        "kind": "stage_receipt",
                        "id": propose_stage_receipt,
                        "stage_call_id": propose.identity.stage_call_id(),
                        "stage_role": "proposer",
                        "consumes": [
                            {
                                "kind": "reflection_result",
                                "fingerprint": reflection.fingerprint(),
                                "receipt": reflect_stage_receipt
                            }
                        ]
                    }
                ]
            }),
        })
    }

    /// Returns the public-seam wire payload.
    #[must_use]
    pub const fn value(&self) -> &Value {
        &self.value
    }

    /// Consumes the handoff and returns the public-seam wire payload.
    #[must_use]
    pub fn into_value(self) -> Value {
        self.value
    }
}

fn stage_payload_fingerprint(value: &Value) -> Result<String, PublicStagePayloadError> {
    let digest = jcs_canonicalize::sha256_jcs_hex(value)
        .map_err(|error| PublicStagePayloadError::Fingerprint(error.to_string()))?;
    Ok(format!("fp_stage_payload_sha256_{digest}"))
}

fn non_empty(value: String, field: &'static str) -> Result<String, PublicStagePayloadError> {
    if value.trim().is_empty() {
        Err(PublicStagePayloadError::EmptyField { field })
    } else {
        Ok(value)
    }
}
