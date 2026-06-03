use serde_json::Value;

use crate::{OutputRecordDocument, PublicSeamError, StagePayloadDocument, StagePayloadRole};

/// Stage kind dispatched by one generic `leaven/stage.run` call.
///
/// V1 dispatches target-free runner stages and proposer stages. Reflector,
/// scorer, and judge dispatch lands behind this same generic method as later
/// slices wire their stage payloads and outputs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StageRunKind {
    /// Runner stage: produce a candidate output for a target-free case input.
    Runner,
    /// Proposer stage: submit a typed proposal batch through nested callbacks.
    Proposer,
}

impl StageRunKind {
    fn parse(value: &str) -> Result<Self, PublicSeamError> {
        match value {
            "runner" => Ok(Self::Runner),
            "proposer" => Ok(Self::Proposer),
            other => Err(invalid_stage_run(format!(
                "unknown stage run kind `{other}`"
            ))),
        }
    }

    /// Stage payload role that backs this stage kind.
    const fn payload_role(self) -> StagePayloadRole {
        match self {
            Self::Runner => StagePayloadRole::Runner,
            Self::Proposer => StagePayloadRole::Proposer,
        }
    }

    /// Wire spelling of the stage kind.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Runner => "runner",
            Self::Proposer => "proposer",
        }
    }
}

/// Schema-valid `leaven/stage.run` request: a stage kind plus a role-scoped payload.
///
/// The host dispatches one stage to a worker. V1 carries a target-free
/// `RunnerRequest`; the embedded payload is validated through the same
/// runner-stage semantic checks as a standalone stage payload, so a stage-run
/// dispatch cannot smuggle case-target material past the runner-stage guard.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StageRunRequestDocument {
    stage: StageRunKind,
    payload: StagePayloadDocument,
}

impl StageRunRequestDocument {
    pub(crate) fn from_schema_valid_value(value: &Value) -> Result<Self, PublicSeamError> {
        let object = value
            .as_object()
            .ok_or_else(|| invalid_stage_run("stage run request must be an object"))?;
        require_message(object.get("message"), "stage_run_request")?;
        let stage = StageRunKind::parse(required_str(object.get("stage"), "stage")?)?;
        let payload_value = object
            .get("payload")
            .ok_or_else(|| invalid_stage_run("stage run request must carry a payload"))?;
        let payload = StagePayloadDocument::from_schema_valid_value(payload_value)
            .map_err(rewrap_payload_error)?;
        if payload.role() != stage.payload_role() {
            return Err(invalid_stage_run(format!(
                "stage run `{}` must carry a `{}` payload",
                stage.as_str(),
                stage.payload_role().as_str()
            )));
        }
        Ok(Self { stage, payload })
    }

    /// Stage kind dispatched by this request.
    pub const fn stage(&self) -> StageRunKind {
        self.stage
    }

    /// Role-scoped stage payload carried by this request.
    pub const fn payload(&self) -> &StagePayloadDocument {
        &self.payload
    }
}

/// Schema-valid `leaven/stage.run` result: the dispatched stage's typed output.
///
/// V1 returns a stage `OutputRecord` of kind `text`. The output reuses the
/// locked `OutputRecord` semantics, so a stage-run result cannot return a
/// shapeless blob in place of a reportable output.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StageRunResultDocument {
    stage: StageRunKind,
    stage_call_id: String,
    output: OutputRecordDocument,
    effect_receipts: Vec<StageEffectReceipt>,
}

/// Opaque effect receipt reported by a worker while producing a stage result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StageEffectReceipt {
    method: String,
    receipt: String,
    call_kind: Option<String>,
}

impl StageEffectReceipt {
    fn from_schema_valid_value(value: &Value) -> Result<Self, PublicSeamError> {
        let object = value
            .as_object()
            .ok_or_else(|| invalid_stage_run("stage effect receipt must be an object"))?;
        let method = required_str(object.get("method"), "effect_receipts.method")?.to_owned();
        let receipt = required_str(object.get("receipt"), "effect_receipts.receipt")?.to_owned();
        let call_kind = optional_str(object.get("call_kind"), "effect_receipts.call_kind")?;
        validate_effect_receipt_binding(&method, &receipt, call_kind)?;
        Ok(Self {
            method,
            receipt,
            call_kind: call_kind.map(ToOwned::to_owned),
        })
    }

    /// Worker callback method that produced this receipt.
    pub fn method(&self) -> &str {
        &self.method
    }

    /// Opaque effect receipt id.
    pub fn receipt(&self) -> &str {
        &self.receipt
    }

    /// Optional receipt family label from the callback result.
    pub fn call_kind(&self) -> Option<&str> {
        self.call_kind.as_deref()
    }
}

impl StageRunResultDocument {
    pub(crate) fn from_schema_valid_value(value: &Value) -> Result<Self, PublicSeamError> {
        let object = value
            .as_object()
            .ok_or_else(|| invalid_stage_run("stage run result must be an object"))?;
        require_message(object.get("message"), "stage_run_result")?;
        let stage = StageRunKind::parse(required_str(object.get("stage"), "stage")?)?;
        let stage_call_id = required_str(object.get("stage_call_id"), "stage_call_id")?.to_owned();
        let output_value = object
            .get("output")
            .ok_or_else(|| invalid_stage_run("stage run result must carry an output"))?;
        let output = OutputRecordDocument::from_schema_valid_value(output_value.clone())
            .map_err(rewrap_output_error)?;
        if output.kind() != "text" {
            return Err(invalid_stage_run(
                "V1 stage run result output must be kind `text`",
            ));
        }
        let effect_receipts = effect_receipts(object.get("effect_receipts"))?;
        Ok(Self {
            stage,
            stage_call_id,
            output,
            effect_receipts,
        })
    }

    /// Stage kind answered by this result.
    pub const fn stage(&self) -> StageRunKind {
        self.stage
    }

    /// Stage call id this result answers.
    pub fn stage_call_id(&self) -> &str {
        &self.stage_call_id
    }

    /// Typed stage output returned by the worker.
    pub const fn output(&self) -> &OutputRecordDocument {
        &self.output
    }

    /// Effect receipts reported by worker callbacks while producing this output.
    pub fn effect_receipts(&self) -> &[StageEffectReceipt] {
        &self.effect_receipts
    }
}

fn require_message(value: Option<&Value>, expected: &str) -> Result<(), PublicSeamError> {
    match value.and_then(Value::as_str) {
        Some(message) if message == expected => Ok(()),
        _ => Err(invalid_stage_run(format!(
            "stage run document must declare message `{expected}`"
        ))),
    }
}

fn required_str<'a>(value: Option<&'a Value>, field: &str) -> Result<&'a str, PublicSeamError> {
    value
        .and_then(Value::as_str)
        .ok_or_else(|| invalid_stage_run(format!("stage run field `{field}` must be a string")))
}

fn optional_str<'a>(
    value: Option<&'a Value>,
    field: &str,
) -> Result<Option<&'a str>, PublicSeamError> {
    value
        .map(|value| {
            value.as_str().ok_or_else(|| {
                invalid_stage_run(format!("stage run field `{field}` must be a string"))
            })
        })
        .transpose()
}

fn effect_receipts(value: Option<&Value>) -> Result<Vec<StageEffectReceipt>, PublicSeamError> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let receipts = value
        .as_array()
        .ok_or_else(|| invalid_stage_run("effect_receipts must be an array"))?;
    receipts
        .iter()
        .map(StageEffectReceipt::from_schema_valid_value)
        .collect()
}

fn validate_effect_receipt_binding(
    method: &str,
    receipt: &str,
    call_kind: Option<&str>,
) -> Result<(), PublicSeamError> {
    let (expected_prefix, expected_kind) = match method {
        "leaven/lm.complete" => ("lmrec_", "lm_complete"),
        "leaven/agent.run" => ("agentrec_", "agent_run"),
        other => {
            return Err(invalid_stage_run(format!(
                "effect_receipts.method `{other}` is not an effect callback method"
            )));
        }
    };
    if !receipt.starts_with(expected_prefix) {
        return Err(invalid_stage_run(format!(
            "effect receipt `{receipt}` does not match method `{method}`"
        )));
    }
    if call_kind.is_some_and(|kind| kind != expected_kind) {
        return Err(invalid_stage_run(format!(
            "effect receipt call_kind must be `{expected_kind}` for method `{method}`"
        )));
    }
    Ok(())
}

fn rewrap_payload_error(error: PublicSeamError) -> PublicSeamError {
    match error {
        PublicSeamError::InvalidStagePayload { message } => invalid_stage_run(format!(
            "stage run payload is not valid for the requested stage: {message}"
        )),
        other => other,
    }
}

fn rewrap_output_error(error: PublicSeamError) -> PublicSeamError {
    match error {
        PublicSeamError::InvalidOutputRecord { message } => {
            invalid_stage_run(format!("stage run result output is invalid: {message}"))
        }
        other => other,
    }
}

fn invalid_stage_run(message: impl Into<String>) -> PublicSeamError {
    PublicSeamError::InvalidStageRun {
        message: message.into(),
    }
}
