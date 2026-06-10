use base64::{Engine as _, engine::general_purpose};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::{
    LockedMethod, OutputRecordDocument, PublicSeamError, StagePayloadDocument, StagePayloadRole,
};

/// Stage kind dispatched by one generic `leaven/stage.run` call.
///
/// V1 dispatches target-free runner stages, scorer stages, and proposer stages.
/// Reflector and judge dispatch lands behind this same generic method as later
/// slices wire their stage payloads and outputs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StageRunKind {
    /// Runner stage: produce a candidate output for a target-free case input.
    Runner,
    /// Scorer stage: score a candidate output for a case and return a reward vector.
    Scorer,
    /// Proposer stage: submit a typed proposal batch through nested callbacks.
    Proposer,
}

impl StageRunKind {
    fn parse(value: &str) -> Result<Self, PublicSeamError> {
        match value {
            "runner" => Ok(Self::Runner),
            "scorer" => Ok(Self::Scorer),
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
            Self::Scorer => StagePayloadRole::Scorer,
            Self::Proposer => StagePayloadRole::Proposer,
        }
    }

    /// Wire spelling of the stage kind.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Runner => "runner",
            Self::Scorer => "scorer",
            Self::Proposer => "proposer",
        }
    }

    /// Whether a stage result of this kind must carry a reward-vector score.
    const fn requires_score(self) -> bool {
        matches!(self, Self::Scorer)
    }
}

/// Schema-valid `leaven/stage.run` request: a stage kind plus a role-scoped payload.
///
/// The host dispatches one stage to a worker. V1 carries a runner, scorer, or
/// proposer payload; the embedded payload is re-validated through the same
/// role-scoped stage-payload semantic checks as a standalone stage payload, so
/// the dispatched payload role must match the stage kind. The runner-stage
/// guard keeps case-target material out of runner dispatch; scorer dispatch
/// deliberately permits case-target access, which is mediated by capability-
/// gated case reads rather than the stage-payload guard.
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
#[derive(Clone, Debug, PartialEq)]
pub struct StageRunResultDocument {
    stage: StageRunKind,
    stage_call_id: String,
    output: OutputRecordDocument,
    score: Option<StageScoreFact>,
    effect_receipts: Vec<StageEffectReceipt>,
    proposal_receipts: Vec<StageProposalReceipt>,
}

/// Reward-vector score returned by a scorer stage dispatch.
///
/// The scalar `value` is the collapsed reward, and `rewards` carry the
/// per-reward vector the optimizer reflects over. Reward and score numbers are
/// validated finite, so a stage cannot smuggle `NaN`/infinite reward facts.
#[derive(Clone, Debug, PartialEq)]
pub struct StageScoreFact {
    value: f64,
    rewards: Vec<StageRewardFact>,
}

impl StageScoreFact {
    fn from_schema_valid_value(value: &Value) -> Result<Self, PublicSeamError> {
        let object = value
            .as_object()
            .ok_or_else(|| invalid_stage_run("stage run score must be an object"))?;
        let scalar = required_finite_number(object.get("value"), "score.value")?;
        let rewards = object
            .get("rewards")
            .and_then(Value::as_array)
            .ok_or_else(|| invalid_stage_run("stage run score must carry a rewards array"))?
            .iter()
            .map(StageRewardFact::from_schema_valid_value)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            value: scalar,
            rewards,
        })
    }

    /// Collapsed scalar score returned by the scorer stage.
    pub const fn value(&self) -> f64 {
        self.value
    }

    /// Per-reward vector backing the collapsed score.
    pub fn rewards(&self) -> &[StageRewardFact] {
        &self.rewards
    }
}

/// One reward in a scorer-stage reward vector.
#[derive(Clone, Debug, PartialEq)]
pub struct StageRewardFact {
    id: String,
    value: f64,
    weight: f64,
    feedback: Option<String>,
}

impl StageRewardFact {
    fn from_schema_valid_value(value: &Value) -> Result<Self, PublicSeamError> {
        let object = value
            .as_object()
            .ok_or_else(|| invalid_stage_run("stage run reward must be an object"))?;
        Ok(Self {
            id: required_str(object.get("id"), "score.rewards.id")?.to_owned(),
            value: required_finite_number(object.get("value"), "score.rewards.value")?,
            weight: required_finite_number(object.get("weight"), "score.rewards.weight")?,
            feedback: optional_str(object.get("feedback"), "score.rewards.feedback")?
                .map(ToOwned::to_owned),
        })
    }

    /// Reward id.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Reward value.
    pub const fn value(&self) -> f64 {
        self.value
    }

    /// Reward weight.
    pub const fn weight(&self) -> f64 {
        self.weight
    }

    /// Optional human-readable reward feedback.
    pub fn feedback(&self) -> Option<&str> {
        self.feedback.as_deref()
    }
}

/// Effect receipt reported by a worker while producing a stage result.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StageEffectReceipt {
    method: LockedMethod,
    receipt: String,
    call_kind: Option<String>,
    cost: Option<StageEffectCostFact>,
    blob_refs: Vec<StageEffectBlobRefFact>,
    blob_contents: Vec<StageEffectBlobContent>,
}

/// Metered cost counters reported by a stage effect callback.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StageEffectCostFact {
    usd_micro: Option<u64>,
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
    lm_calls: Option<u64>,
}

impl StageEffectCostFact {
    fn from_schema_valid_value(value: &Value, field: &str) -> Result<Self, PublicSeamError> {
        let object = value.as_object().ok_or_else(|| {
            invalid_stage_run(format!("stage run field `{field}` must be an object"))
        })?;
        Ok(Self {
            usd_micro: optional_u64(object.get("usd_micro"), "effect_receipts.cost.usd_micro")?,
            input_tokens: optional_u64(
                object.get("input_tokens"),
                "effect_receipts.cost.input_tokens",
            )?,
            output_tokens: optional_u64(
                object.get("output_tokens"),
                "effect_receipts.cost.output_tokens",
            )?,
            lm_calls: optional_u64(object.get("lm_calls"), "effect_receipts.cost.lm_calls")?,
        })
    }

    /// USD-denominated cost in millionths of a dollar, if reported.
    pub const fn usd_micro(&self) -> Option<u64> {
        self.usd_micro
    }

    /// Prompt/input token count, if reported.
    pub const fn input_tokens(&self) -> Option<u64> {
        self.input_tokens
    }

    /// Completion/output token count, if reported.
    pub const fn output_tokens(&self) -> Option<u64> {
        self.output_tokens
    }

    /// Number of LM calls charged by this effect, if reported.
    pub const fn lm_calls(&self) -> Option<u64> {
        self.lm_calls
    }
}

/// Blob reference audit metadata reported by a stage effect callback.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StageEffectBlobRefFact {
    id: String,
    sha256: String,
    bytes: u64,
    media_type: Option<String>,
    uri: Option<String>,
    data_classes: Vec<String>,
}

/// Byte content bound to a worker callback blob ref.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StageEffectBlobContent {
    blob_ref: StageEffectBlobRefFact,
    content_base64: String,
}

impl StageEffectBlobContent {
    fn from_schema_valid_value(value: &Value, field: &str) -> Result<Self, PublicSeamError> {
        let object = value.as_object().ok_or_else(|| {
            invalid_stage_run(format!("stage run field `{field}` must be an object"))
        })?;
        let blob_ref = StageEffectBlobRefFact::from_schema_valid_value(
            object.get("blob_ref").ok_or_else(|| {
                invalid_stage_run("effect_receipts.blob_contents.blob_ref is required")
            })?,
            "effect_receipts.blob_contents.blob_ref",
        )?;
        let content_base64 = required_str(
            object.get("content_base64"),
            "effect_receipts.blob_contents.content_base64",
        )?
        .to_owned();
        validate_blob_content(&blob_ref, &content_base64)?;
        Ok(Self {
            blob_ref,
            content_base64,
        })
    }

    /// Blob ref whose bytes are supplied.
    pub const fn blob_ref(&self) -> &StageEffectBlobRefFact {
        &self.blob_ref
    }

    /// Base64-encoded blob bytes.
    pub fn content_base64(&self) -> &str {
        &self.content_base64
    }

    /// Decode the supplied blob bytes.
    pub fn content_bytes(&self) -> Result<Vec<u8>, PublicSeamError> {
        general_purpose::STANDARD
            .decode(&self.content_base64)
            .map_err(|error| {
                invalid_stage_run(format!(
                    "effect_receipts.blob_contents.content_base64 is invalid: {error}"
                ))
            })
    }
}

impl StageEffectBlobRefFact {
    fn from_schema_valid_value(value: &Value, field: &str) -> Result<Self, PublicSeamError> {
        let object = value.as_object().ok_or_else(|| {
            invalid_stage_run(format!("stage run field `{field}` must be an object"))
        })?;
        match required_str(object.get("kind"), "effect_receipts.blob_refs.kind")? {
            "blob_ref" => {}
            other => {
                return Err(invalid_stage_run(format!(
                    "effect_receipts.blob_refs.kind must be `blob_ref`, got `{other}`"
                )));
            }
        }
        Ok(Self {
            id: required_str(object.get("id"), "effect_receipts.blob_refs.id")?.to_owned(),
            sha256: required_str(object.get("sha256"), "effect_receipts.blob_refs.sha256")?
                .to_owned(),
            bytes: required_u64(object.get("bytes"), "effect_receipts.blob_refs.bytes")?,
            media_type: optional_str(
                object.get("media_type"),
                "effect_receipts.blob_refs.media_type",
            )?
            .map(ToOwned::to_owned),
            uri: optional_str(object.get("uri"), "effect_receipts.blob_refs.uri")?
                .map(ToOwned::to_owned),
            data_classes: required_string_array(
                object.get("data_classes"),
                "effect_receipts.blob_refs.data_classes",
            )?,
        })
    }

    /// Public blob id.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Lowercase SHA-256 digest for the referenced bytes.
    pub fn sha256(&self) -> &str {
        &self.sha256
    }

    /// Referenced byte length.
    pub const fn bytes(&self) -> u64 {
        self.bytes
    }

    /// Optional media type metadata.
    pub fn media_type(&self) -> Option<&str> {
        self.media_type.as_deref()
    }

    /// Optional retrievable URI.
    pub fn uri(&self) -> Option<&str> {
        self.uri.as_deref()
    }

    /// Data classes carried by the referenced blob.
    pub fn data_classes(&self) -> &[String] {
        &self.data_classes
    }
}

impl StageEffectReceipt {
    fn from_schema_valid_value(value: &Value) -> Result<Self, PublicSeamError> {
        let object = value
            .as_object()
            .ok_or_else(|| invalid_stage_run("stage effect receipt must be an object"))?;
        let method_name = required_str(object.get("method"), "effect_receipts.method")?;
        let method = LockedMethod::parse(method_name).ok_or_else(|| {
            invalid_stage_run(format!(
                "effect_receipts.method `{method_name}` is not a locked callback method"
            ))
        })?;
        let receipt = required_str(object.get("receipt"), "effect_receipts.receipt")?.to_owned();
        let call_kind = optional_str(object.get("call_kind"), "effect_receipts.call_kind")?;
        let cost = optional_cost_fact(object.get("cost"), "effect_receipts.cost")?;
        let blob_refs = blob_ref_facts(object.get("blob_refs"), "effect_receipts.blob_refs")?;
        let blob_contents =
            blob_contents(object.get("blob_contents"), "effect_receipts.blob_contents")?;
        validate_effect_receipt_binding(method, &receipt, call_kind)?;
        validate_blob_content_refs(&blob_refs, &blob_contents)?;
        Ok(Self {
            method,
            receipt,
            call_kind: call_kind.map(ToOwned::to_owned),
            cost,
            blob_refs,
            blob_contents,
        })
    }

    /// Worker callback method that produced this receipt.
    pub const fn method(&self) -> LockedMethod {
        self.method
    }

    /// Opaque effect receipt id.
    pub fn receipt(&self) -> &str {
        &self.receipt
    }

    /// Optional receipt family label from the callback result.
    pub fn call_kind(&self) -> Option<&str> {
        self.call_kind.as_deref()
    }

    /// Optional metered cost reported by the callback primary value.
    pub fn cost(&self) -> Option<&StageEffectCostFact> {
        self.cost.as_ref()
    }

    /// Blob references reported by the callback primary value.
    pub fn blob_refs(&self) -> &[StageEffectBlobRefFact] {
        &self.blob_refs
    }

    /// Blob byte contents reported by the callback primary value.
    pub fn blob_contents(&self) -> &[StageEffectBlobContent] {
        &self.blob_contents
    }
}

/// Opaque proposal write receipt reported by a proposer worker.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StageProposalReceipt {
    method: LockedMethod,
    receipt: String,
    write_kind: Option<String>,
    proposal_ids: Vec<String>,
}

impl StageProposalReceipt {
    fn from_schema_valid_value(value: &Value) -> Result<Self, PublicSeamError> {
        let object = value
            .as_object()
            .ok_or_else(|| invalid_stage_run("stage proposal receipt must be an object"))?;
        let method_name = required_str(object.get("method"), "proposal_receipts.method")?;
        let method = LockedMethod::parse(method_name).ok_or_else(|| {
            invalid_stage_run(format!(
                "proposal_receipts.method `{method_name}` is not a locked callback method"
            ))
        })?;
        let receipt = required_receipt_id(object.get("receipt"), "proposal_receipts.receipt")?;
        let write_kind = optional_str(object.get("write_kind"), "proposal_receipts.write_kind")?;
        let proposal_ids = proposal_ids(object.get("proposal_ids"))?;
        validate_proposal_receipt_binding(method, &receipt, write_kind)?;
        Ok(Self {
            method,
            receipt,
            write_kind: write_kind.map(ToOwned::to_owned),
            proposal_ids,
        })
    }

    /// Worker callback method that produced this receipt.
    pub const fn method(&self) -> LockedMethod {
        self.method
    }

    /// Opaque proposal write receipt id.
    pub fn receipt(&self) -> &str {
        &self.receipt
    }

    /// Optional receipt family label from the callback result.
    pub fn write_kind(&self) -> Option<&str> {
        self.write_kind.as_deref()
    }

    /// Proposal ids reported by the proposal batch receipt primary value.
    pub fn proposal_ids(&self) -> &[String] {
        &self.proposal_ids
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
        let score = object
            .get("score")
            .map(StageScoreFact::from_schema_valid_value)
            .transpose()?;
        if stage.requires_score() && score.is_none() {
            return Err(invalid_stage_run(format!(
                "stage run `{}` result must carry a reward-vector score",
                stage.as_str()
            )));
        }
        if !stage.requires_score() && score.is_some() {
            return Err(invalid_stage_run(format!(
                "stage run `{}` result must not carry a score",
                stage.as_str()
            )));
        }
        let effect_receipts = effect_receipts(object.get("effect_receipts"))?;
        let proposal_receipts = proposal_receipts(object.get("proposal_receipts"))?;
        Ok(Self {
            stage,
            stage_call_id,
            output,
            score,
            effect_receipts,
            proposal_receipts,
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

    /// Reward-vector score returned by a scorer stage, if any.
    ///
    /// A scorer-stage result always carries a score; runner and proposer results
    /// never do.
    pub const fn score(&self) -> Option<&StageScoreFact> {
        self.score.as_ref()
    }

    /// Effect receipts reported by worker callbacks while producing this output.
    pub fn effect_receipts(&self) -> &[StageEffectReceipt] {
        &self.effect_receipts
    }

    /// Proposal write receipts reported by proposer-stage callbacks.
    pub fn proposal_receipts(&self) -> &[StageProposalReceipt] {
        &self.proposal_receipts
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

fn optional_cost_fact(
    value: Option<&Value>,
    field: &str,
) -> Result<Option<StageEffectCostFact>, PublicSeamError> {
    value
        .map(|value| StageEffectCostFact::from_schema_valid_value(value, field))
        .transpose()
}

fn blob_ref_facts(
    value: Option<&Value>,
    field: &str,
) -> Result<Vec<StageEffectBlobRefFact>, PublicSeamError> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let values = value
        .as_array()
        .ok_or_else(|| invalid_stage_run(format!("stage run field `{field}` must be an array")))?;
    values
        .iter()
        .map(|value| StageEffectBlobRefFact::from_schema_valid_value(value, field))
        .collect()
}

fn blob_contents(
    value: Option<&Value>,
    field: &str,
) -> Result<Vec<StageEffectBlobContent>, PublicSeamError> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let values = value
        .as_array()
        .ok_or_else(|| invalid_stage_run(format!("stage run field `{field}` must be an array")))?;
    values
        .iter()
        .map(|value| StageEffectBlobContent::from_schema_valid_value(value, field))
        .collect()
}

fn validate_blob_content(
    blob_ref: &StageEffectBlobRefFact,
    content_base64: &str,
) -> Result<(), PublicSeamError> {
    let bytes = general_purpose::STANDARD
        .decode(content_base64)
        .map_err(|error| {
            invalid_stage_run(format!(
                "effect_receipts.blob_contents.content_base64 is invalid: {error}"
            ))
        })?;
    let actual_len = u64::try_from(bytes.len()).map_err(|_| {
        invalid_stage_run("effect_receipts.blob_contents content is too large for byte audit")
    })?;
    if blob_ref.bytes != actual_len {
        return Err(invalid_stage_run(format!(
            "effect_receipts.blob_contents blob_ref bytes `{}` do not match content bytes `{actual_len}`",
            blob_ref.bytes
        )));
    }
    let actual_sha = format!("{:x}", Sha256::digest(&bytes));
    if blob_ref.sha256 != actual_sha {
        return Err(invalid_stage_run(
            "effect_receipts.blob_contents blob_ref sha256 does not match content",
        ));
    }
    Ok(())
}

fn validate_blob_content_refs(
    blob_refs: &[StageEffectBlobRefFact],
    contents: &[StageEffectBlobContent],
) -> Result<(), PublicSeamError> {
    for content in contents {
        if !blob_refs.iter().any(|reference| {
            reference.id == content.blob_ref.id
                && reference.sha256 == content.blob_ref.sha256
                && reference.bytes == content.blob_ref.bytes
        }) {
            return Err(invalid_stage_run(format!(
                "effect_receipts.blob_contents ref `{}` must also appear in blob_refs",
                content.blob_ref.id
            )));
        }
    }
    Ok(())
}

fn required_receipt_id(value: Option<&Value>, field: &str) -> Result<String, PublicSeamError> {
    match value {
        Some(Value::String(receipt)) => Ok(receipt.to_owned()),
        Some(Value::Object(object)) => required_str(object.get("id"), field).map(ToOwned::to_owned),
        _ => Err(invalid_stage_run(format!(
            "stage run field `{field}` must be a receipt ref"
        ))),
    }
}

fn required_u64(value: Option<&Value>, field: &str) -> Result<u64, PublicSeamError> {
    value
        .and_then(Value::as_u64)
        .ok_or_else(|| invalid_stage_run(format!("stage run field `{field}` must be a u64")))
}

fn required_finite_number(value: Option<&Value>, field: &str) -> Result<f64, PublicSeamError> {
    let number = value
        .and_then(Value::as_f64)
        .ok_or_else(|| invalid_stage_run(format!("stage run field `{field}` must be a number")))?;
    if !number.is_finite() {
        return Err(invalid_stage_run(format!(
            "stage run field `{field}` must be finite"
        )));
    }
    Ok(number)
}

fn optional_u64(value: Option<&Value>, field: &str) -> Result<Option<u64>, PublicSeamError> {
    value
        .map(|value| {
            value.as_u64().ok_or_else(|| {
                invalid_stage_run(format!("stage run field `{field}` must be a u64"))
            })
        })
        .transpose()
}

fn required_string_array(
    value: Option<&Value>,
    field: &str,
) -> Result<Vec<String>, PublicSeamError> {
    let value = value
        .ok_or_else(|| invalid_stage_run(format!("stage run field `{field}` must be an array")))?;
    let values = value
        .as_array()
        .ok_or_else(|| invalid_stage_run(format!("stage run field `{field}` must be an array")))?;
    values
        .iter()
        .map(|value| {
            value.as_str().map(ToOwned::to_owned).ok_or_else(|| {
                invalid_stage_run(format!(
                    "stage run field `{field}` must contain only strings"
                ))
            })
        })
        .collect()
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

fn proposal_receipts(value: Option<&Value>) -> Result<Vec<StageProposalReceipt>, PublicSeamError> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let receipts = value
        .as_array()
        .ok_or_else(|| invalid_stage_run("proposal_receipts must be an array"))?;
    receipts
        .iter()
        .map(StageProposalReceipt::from_schema_valid_value)
        .collect()
}

fn proposal_ids(value: Option<&Value>) -> Result<Vec<String>, PublicSeamError> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let ids = value
        .as_array()
        .ok_or_else(|| invalid_stage_run("proposal_receipts.proposal_ids must be an array"))?;
    ids.iter()
        .map(|value| {
            value
                .as_str()
                .map(ToOwned::to_owned)
                .ok_or_else(|| invalid_stage_run("proposal id must be a string"))
        })
        .collect()
}

fn validate_effect_receipt_binding(
    method: LockedMethod,
    receipt: &str,
    call_kind: Option<&str>,
) -> Result<(), PublicSeamError> {
    let (expected_prefix, expected_kind) = match method {
        LockedMethod::LmComplete => ("lmrec_", "lm_complete"),
        LockedMethod::AgentRun => ("agentrec_", "agent_run"),
        other => {
            return Err(invalid_stage_run(format!(
                "effect_receipts.method `{}` is not an effect callback method",
                other.as_str()
            )));
        }
    };
    if !receipt.starts_with(expected_prefix) {
        return Err(invalid_stage_run(format!(
            "effect receipt `{receipt}` does not match method `{}`",
            method.as_str()
        )));
    }
    if call_kind.is_some_and(|kind| kind != expected_kind) {
        return Err(invalid_stage_run(format!(
            "effect receipt call_kind must be `{expected_kind}` for method `{}`",
            method.as_str()
        )));
    }
    Ok(())
}

fn validate_proposal_receipt_binding(
    method: LockedMethod,
    receipt: &str,
    write_kind: Option<&str>,
) -> Result<(), PublicSeamError> {
    if method != LockedMethod::ProposalSubmitBatch {
        return Err(invalid_stage_run(format!(
            "proposal_receipts.method `{}` is not a proposal callback method",
            method.as_str()
        )));
    }
    if !receipt.starts_with("wrec_") {
        return Err(invalid_stage_run(format!(
            "proposal receipt `{receipt}` does not match method `{}`",
            method.as_str()
        )));
    }
    if write_kind.is_some_and(|kind| kind != "submit_proposal_batch") {
        return Err(invalid_stage_run(
            "proposal receipt write_kind must be `submit_proposal_batch`",
        ));
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
