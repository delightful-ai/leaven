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
        for example in &examples {
            reject_case_target_material(example, "examples")?;
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

/// Public-seam `RunnerRequest` payload for target-free runner execution.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunnerRequestPayload {
    value: Value,
}

impl RunnerRequestPayload {
    /// Builds a target-free runner request payload.
    pub fn new(
        run: impl Into<String>,
        stage_call_id: impl Into<String>,
        candidate: impl Into<String>,
        case_ref: impl Into<String>,
        case_input: Value,
    ) -> Result<Self, PublicStagePayloadError> {
        reject_case_target_material(&case_input, "case_input")?;
        let mut object = Map::new();
        object.insert(
            "schema_version".to_owned(),
            json!("leaven.stage_payloads.v1"),
        );
        object.insert("role".to_owned(), json!("runner"));
        object.insert("run".to_owned(), json!(non_empty(run.into(), "run")?));
        object.insert(
            "stage_call_id".to_owned(),
            json!(non_empty(stage_call_id.into(), "stage_call_id")?),
        );
        object.insert(
            "candidate".to_owned(),
            json!(non_empty(candidate.into(), "candidate")?),
        );
        object.insert(
            "case".to_owned(),
            json!(non_empty(case_ref.into(), "case")?),
        );
        object.insert("case_input".to_owned(), case_input);
        object.insert("target_forbidden".to_owned(), Value::Bool(true));
        Ok(Self {
            value: Value::Object(object),
        })
    }

    /// Returns the public-seam wire payload.
    #[must_use]
    pub const fn value(&self) -> &Value {
        &self.value
    }
}

/// Public-seam `ScoreContext` payload for scorer execution.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScorerContextPayload {
    value: Value,
}

/// Constructor fields for [`ScorerContextPayload`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScorerContextPayloadFields {
    /// Public run reference.
    pub run: String,
    /// Stage call id for the scorer invocation.
    pub stage_call_id: String,
    /// Evaluation request being scored.
    pub evaluation_request_id: String,
    /// Candidate being scored.
    pub candidate: String,
    /// Case being scored.
    pub case_ref: String,
    /// Candidate output being assessed.
    pub output: Value,
    /// Optional target handle, only valid when bound to `case_ref`.
    pub target_handle: Option<String>,
    /// Capability fingerprint authorizing scorer access.
    pub capability_fingerprint: String,
}

impl ScorerContextPayload {
    /// Builds a scorer context payload with capability-bound target access.
    pub fn new(fields: ScorerContextPayloadFields) -> Result<Self, PublicStagePayloadError> {
        let case_ref = non_empty(fields.case_ref, "case")?;
        if let Some(target_handle) = fields.target_handle.as_deref()
            && target_handle != case_ref
        {
            return Err(PublicStagePayloadError::TargetHandleMismatch);
        }
        require_assessed_output_class(&fields.output, "output")?;
        let mut object = Map::new();
        object.insert(
            "schema_version".to_owned(),
            json!("leaven.stage_payloads.v1"),
        );
        object.insert("role".to_owned(), json!("scorer"));
        object.insert("run".to_owned(), json!(non_empty(fields.run, "run")?));
        object.insert(
            "stage_call_id".to_owned(),
            json!(non_empty(fields.stage_call_id, "stage_call_id")?),
        );
        object.insert(
            "evaluation_request_id".to_owned(),
            json!(non_empty(
                fields.evaluation_request_id,
                "evaluation_request_id"
            )?),
        );
        object.insert(
            "candidate".to_owned(),
            json!(non_empty(fields.candidate, "candidate")?),
        );
        object.insert("case".to_owned(), json!(case_ref));
        object.insert("output".to_owned(), fields.output);
        if let Some(target_handle) = fields.target_handle {
            object.insert("target_handle".to_owned(), json!(target_handle));
        }
        object.insert(
            "capability_fingerprint".to_owned(),
            json!(non_empty(
                fields.capability_fingerprint,
                "capability_fingerprint"
            )?),
        );
        Ok(Self {
            value: Value::Object(object),
        })
    }

    /// Returns the public-seam wire payload.
    #[must_use]
    pub const fn value(&self) -> &Value {
        &self.value
    }
}

/// Public-seam `JudgeContext` payload for pairwise/listwise judging.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JudgeContextPayload {
    value: Value,
}

/// Constructor fields for [`JudgeContextPayload`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JudgeContextPayloadFields {
    /// Public run reference.
    pub run: String,
    /// Stage call id for the judge invocation.
    pub stage_call_id: String,
    /// Left candidate being compared.
    pub left: String,
    /// Right candidate being compared.
    pub right: String,
    /// Optional case being judged.
    pub case_ref: Option<String>,
    /// Assessed outputs being compared.
    pub outputs: Vec<Value>,
    /// Rubric visible to the judge.
    pub rubric: Value,
    /// Capability fingerprint authorizing judge access.
    pub capability_fingerprint: String,
}

impl JudgeContextPayload {
    /// Builds a judge context payload for assessed candidate outputs.
    pub fn new(fields: JudgeContextPayloadFields) -> Result<Self, PublicStagePayloadError> {
        if fields.outputs.is_empty() {
            return Err(PublicStagePayloadError::EmptyField { field: "outputs" });
        }
        for output in &fields.outputs {
            require_assessed_output_class(output, "outputs")?;
        }
        let mut object = Map::new();
        object.insert(
            "schema_version".to_owned(),
            json!("leaven.stage_payloads.v1"),
        );
        object.insert("role".to_owned(), json!("judge"));
        object.insert("run".to_owned(), json!(non_empty(fields.run, "run")?));
        object.insert(
            "stage_call_id".to_owned(),
            json!(non_empty(fields.stage_call_id, "stage_call_id")?),
        );
        object.insert("left".to_owned(), json!(non_empty(fields.left, "left")?));
        object.insert("right".to_owned(), json!(non_empty(fields.right, "right")?));
        if let Some(case_ref) = fields.case_ref {
            object.insert("case".to_owned(), json!(non_empty(case_ref, "case")?));
        }
        object.insert("outputs".to_owned(), json!(fields.outputs));
        object.insert("rubric".to_owned(), fields.rubric);
        object.insert(
            "capability_fingerprint".to_owned(),
            json!(non_empty(
                fields.capability_fingerprint,
                "capability_fingerprint"
            )?),
        );
        Ok(Self {
            value: Value::Object(object),
        })
    }

    /// Returns the public-seam wire payload.
    #[must_use]
    pub const fn value(&self) -> &Value {
        &self.value
    }
}

/// Public-seam callback payload with a schema-bound event body.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CallbackRequestPayload {
    value: Value,
}

impl CallbackRequestPayload {
    /// Builds a callback request payload.
    pub fn new(
        run: impl Into<String>,
        stage_call_id: impl Into<String>,
        event: Value,
        payload_schema: impl Into<String>,
        capability_fingerprint: impl Into<String>,
    ) -> Result<Self, PublicStagePayloadError> {
        Ok(Self {
            value: schema_bound_payload(
                "callback",
                run,
                stage_call_id,
                "event",
                event,
                payload_schema,
                capability_fingerprint,
            )?,
        })
    }

    /// Returns the public-seam wire payload.
    #[must_use]
    pub const fn value(&self) -> &Value {
        &self.value
    }
}

/// Public-seam adapter role.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdapterPayloadRole {
    /// Artifact adapter payload.
    Artifact,
    /// Dataset adapter payload.
    Dataset,
}

impl AdapterPayloadRole {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Artifact => "artifact_adapter",
            Self::Dataset => "dataset_adapter",
        }
    }
}

/// Public-seam artifact or dataset adapter payload.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AdapterRequestPayload {
    value: Value,
}

impl AdapterRequestPayload {
    /// Builds an artifact adapter request payload.
    pub fn artifact(
        run: impl Into<String>,
        stage_call_id: impl Into<String>,
        payload: Value,
        payload_schema: impl Into<String>,
        capability_fingerprint: impl Into<String>,
    ) -> Result<Self, PublicStagePayloadError> {
        Self::new(
            AdapterPayloadRole::Artifact,
            run,
            stage_call_id,
            payload,
            payload_schema,
            capability_fingerprint,
        )
    }

    /// Builds a dataset adapter request payload.
    pub fn dataset(
        run: impl Into<String>,
        stage_call_id: impl Into<String>,
        payload: Value,
        payload_schema: impl Into<String>,
        capability_fingerprint: impl Into<String>,
    ) -> Result<Self, PublicStagePayloadError> {
        Self::new(
            AdapterPayloadRole::Dataset,
            run,
            stage_call_id,
            payload,
            payload_schema,
            capability_fingerprint,
        )
    }

    fn new(
        role: AdapterPayloadRole,
        run: impl Into<String>,
        stage_call_id: impl Into<String>,
        payload: Value,
        payload_schema: impl Into<String>,
        capability_fingerprint: impl Into<String>,
    ) -> Result<Self, PublicStagePayloadError> {
        Ok(Self {
            value: schema_bound_payload(
                role.as_str(),
                run,
                stage_call_id,
                "payload",
                payload,
                payload_schema,
                capability_fingerprint,
            )?,
        })
    }

    /// Returns the public-seam wire payload.
    #[must_use]
    pub const fn value(&self) -> &Value {
        &self.value
    }
}

fn stage_payload_fingerprint(value: &Value) -> Result<String, PublicStagePayloadError> {
    let digest = jcs_canonicalize::sha256_jcs_hex(value)
        .map_err(|error| PublicStagePayloadError::Fingerprint(error.to_string()))?;
    Ok(format!("fp_stage_payload_sha256_{digest}"))
}

fn schema_bound_payload(
    role: &'static str,
    run: impl Into<String>,
    stage_call_id: impl Into<String>,
    payload_field: &'static str,
    payload: Value,
    payload_schema: impl Into<String>,
    capability_fingerprint: impl Into<String>,
) -> Result<Value, PublicStagePayloadError> {
    let mut object = Map::new();
    object.insert(
        "schema_version".to_owned(),
        json!("leaven.stage_payloads.v1"),
    );
    object.insert("role".to_owned(), json!(role));
    object.insert("run".to_owned(), json!(non_empty(run.into(), "run")?));
    object.insert(
        "stage_call_id".to_owned(),
        json!(non_empty(stage_call_id.into(), "stage_call_id")?),
    );
    object.insert(payload_field.to_owned(), payload);
    object.insert(
        "payload_schema".to_owned(),
        json!(non_empty(payload_schema.into(), "payload_schema")?),
    );
    object.insert(
        "capability_fingerprint".to_owned(),
        json!(non_empty(
            capability_fingerprint.into(),
            "capability_fingerprint"
        )?),
    );
    Ok(Value::Object(object))
}

fn require_assessed_output_class(
    output: &Value,
    field: &'static str,
) -> Result<(), PublicStagePayloadError> {
    let carries_assessed_output = output
        .get("data_classes")
        .and_then(Value::as_array)
        .is_some_and(|classes| {
            classes.iter().any(|class| {
                matches!(
                    class.as_str(),
                    Some("candidate.output" | "candidate.artifact")
                )
            })
        });
    if carries_assessed_output {
        Ok(())
    } else {
        Err(PublicStagePayloadError::MissingAssessedOutputClass { field })
    }
}

fn reject_case_target_material(
    value: &Value,
    field: &'static str,
) -> Result<(), PublicStagePayloadError> {
    match value {
        Value::Object(object) => {
            for (key, nested) in object {
                if contains_case_target_marker(key) {
                    return Err(PublicStagePayloadError::TargetLeakage { field });
                }
                reject_case_target_material(nested, field)?;
            }
            Ok(())
        }
        Value::Array(values) => {
            for nested in values {
                reject_case_target_material(nested, field)?;
            }
            Ok(())
        }
        Value::String(text) if contains_case_target_marker(text) => {
            Err(PublicStagePayloadError::TargetLeakage { field })
        }
        _ => Ok(()),
    }
}

fn contains_case_target_marker(text: &str) -> bool {
    text.to_ascii_lowercase().contains("case.target")
}

fn non_empty(value: String, field: &'static str) -> Result<String, PublicStagePayloadError> {
    if value.trim().is_empty() {
        Err(PublicStagePayloadError::EmptyField { field })
    } else {
        Ok(value)
    }
}
