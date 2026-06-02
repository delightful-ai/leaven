use serde_json::Value;

use crate::{OutputRecordDocument, PublicSeamError, StagePayloadDocument, StagePayloadRole};

/// Stage kind dispatched by one generic `leaven/stage.run` call.
///
/// V1 dispatches the target-free runner stage only; reflector/proposer/scorer/
/// judge dispatch lands behind this same generic method as later slices wire
/// their stage payloads and outputs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StageRunKind {
    /// Runner stage: produce a candidate output for a target-free case input.
    Runner,
}

impl StageRunKind {
    fn parse(value: &str) -> Result<Self, PublicSeamError> {
        match value {
            "runner" => Ok(Self::Runner),
            other => Err(invalid_stage_run(format!(
                "unknown stage run kind `{other}`"
            ))),
        }
    }

    /// Stage payload role that backs this stage kind.
    const fn payload_role(self) -> StagePayloadRole {
        match self {
            Self::Runner => StagePayloadRole::Runner,
        }
    }

    /// Wire spelling of the stage kind.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Runner => "runner",
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
/// V1 returns a runner-stage `OutputRecord` of kind `text`. The output reuses
/// the locked `OutputRecord` semantics, so a stage-run result cannot return a
/// shapeless blob in place of a reportable output.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StageRunResultDocument {
    stage: StageRunKind,
    stage_call_id: String,
    output: OutputRecordDocument,
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
                "V1 runner stage run result output must be kind `text`",
            ));
        }
        Ok(Self {
            stage,
            stage_call_id,
            output,
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

fn rewrap_payload_error(error: PublicSeamError) -> PublicSeamError {
    match error {
        PublicSeamError::InvalidStagePayload { message } => invalid_stage_run(format!(
            "stage run payload is not a valid runner stage payload: {message}"
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
