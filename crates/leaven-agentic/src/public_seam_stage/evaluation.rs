use serde_json::{Value, json};

use super::identity::PublicStagePayloadError;
use super::util::{
    insert_non_empty, non_empty, reject_case_target_material, require_assessed_output_class,
    stage_object,
};

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
        let mut object = stage_object("runner");
        insert_non_empty(&mut object, "run", run)?;
        insert_non_empty(&mut object, "stage_call_id", stage_call_id)?;
        insert_non_empty(&mut object, "candidate", candidate)?;
        insert_non_empty(&mut object, "case", case_ref)?;
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
        let mut object = stage_object("scorer");
        insert_non_empty(&mut object, "run", fields.run)?;
        insert_non_empty(&mut object, "stage_call_id", fields.stage_call_id)?;
        insert_non_empty(
            &mut object,
            "evaluation_request_id",
            fields.evaluation_request_id,
        )?;
        insert_non_empty(&mut object, "candidate", fields.candidate)?;
        object.insert("case".to_owned(), json!(case_ref));
        object.insert("output".to_owned(), fields.output);
        if let Some(target_handle) = fields.target_handle {
            object.insert("target_handle".to_owned(), json!(target_handle));
        }
        insert_non_empty(
            &mut object,
            "capability_fingerprint",
            fields.capability_fingerprint,
        )?;
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
        let mut object = stage_object("judge");
        insert_non_empty(&mut object, "run", fields.run)?;
        insert_non_empty(&mut object, "stage_call_id", fields.stage_call_id)?;
        insert_non_empty(&mut object, "left", fields.left)?;
        insert_non_empty(&mut object, "right", fields.right)?;
        if let Some(case_ref) = fields.case_ref {
            insert_non_empty(&mut object, "case", case_ref)?;
        }
        object.insert("outputs".to_owned(), json!(fields.outputs));
        object.insert("rubric".to_owned(), fields.rubric);
        insert_non_empty(
            &mut object,
            "capability_fingerprint",
            fields.capability_fingerprint,
        )?;
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
