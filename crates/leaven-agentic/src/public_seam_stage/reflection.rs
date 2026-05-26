use serde_json::{Value, json};

use super::identity::{PublicStagePayloadError, PublicStagePayloadIdentity};
use super::util::{
    non_empty, reject_case_target_material, stage_object, stage_payload_fingerprint,
};

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
        for example in &examples {
            reject_case_target_material(example, "examples")?;
        }
        let part_label = non_empty(part_label.into(), "part_label")?;
        let mut object = stage_object("reflector");
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
        let mut object = stage_object("proposer");
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
