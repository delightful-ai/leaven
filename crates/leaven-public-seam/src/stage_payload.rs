use serde_json::Value;

use crate::PublicSeamError;

/// Schema-valid public-seam stage payload with role-specific semantic checks.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StagePayloadDocument {
    role: StagePayloadRole,
    source_ref_count: usize,
    read_receipt_count: usize,
    data_classes: Vec<String>,
    capability_fingerprint: Option<String>,
    query_policy_fingerprint: Option<String>,
    reflective_example_count: usize,
    allowed_effects: Vec<StageProposalEffect>,
    allowed_change_schema_count: usize,
    output_count: usize,
    payload_schema: Option<String>,
}

impl StagePayloadDocument {
    pub(crate) fn from_schema_valid_value(value: &Value) -> Result<Self, PublicSeamError> {
        let object = value
            .as_object()
            .ok_or_else(|| invalid_stage_payload("stage payload must be an object"))?;
        let role = StagePayloadRole::parse(required_string(object.get("role"), "role")?)?;
        let source_ref_count = array_len(object.get("source_refs"), "source_refs")?;
        let read_receipt_count = array_len(object.get("read_receipts"), "read_receipts")?;
        let data_classes = string_array(object.get("data_classes"), "data_classes")?;
        let capability_fingerprint = optional_string(object.get("capability_fingerprint"))?;
        let query_policy_fingerprint = optional_string(object.get("query_policy_fingerprint"))?;
        let reflective_example_count = match role {
            StagePayloadRole::Reflector => inspect_reflect_request(object)?,
            _ => 0,
        };
        let (allowed_effects, allowed_change_schema_count) = match role {
            StagePayloadRole::Proposer => inspect_propose_request(object)?,
            _ => (Vec::new(), 0),
        };
        match role {
            StagePayloadRole::ReflectionResult => {
                inspect_reflection_result(
                    object,
                    source_ref_count,
                    read_receipt_count,
                    &data_classes,
                )?;
            }
            StagePayloadRole::Runner => inspect_runner_request(object)?,
            StagePayloadRole::Scorer => inspect_score_context(object)?,
            StagePayloadRole::Judge => inspect_judge_context(object)?,
            StagePayloadRole::Callback
            | StagePayloadRole::ArtifactAdapter
            | StagePayloadRole::DatasetAdapter => {
                inspect_schema_bound_payload(object)?;
            }
            StagePayloadRole::Reflector | StagePayloadRole::Proposer => {}
        }
        let output_count = array_len(object.get("outputs"), "outputs")?
            + usize::from(object.get("output").is_some());
        let payload_schema = optional_string(object.get("payload_schema"))?;

        Ok(Self {
            role,
            source_ref_count,
            read_receipt_count,
            data_classes,
            capability_fingerprint,
            query_policy_fingerprint,
            reflective_example_count,
            allowed_effects,
            allowed_change_schema_count,
            output_count,
            payload_schema,
        })
    }

    /// Stage payload role.
    pub const fn role(&self) -> StagePayloadRole {
        self.role
    }

    /// Number of source refs carried by the payload.
    pub const fn source_ref_count(&self) -> usize {
        self.source_ref_count
    }

    /// Number of read receipts carried by the payload.
    pub const fn read_receipt_count(&self) -> usize {
        self.read_receipt_count
    }

    /// Data classes carried by the payload.
    pub fn data_classes(&self) -> &[String] {
        &self.data_classes
    }

    /// Capability fingerprint authorizing the stage payload when present.
    pub fn capability_fingerprint(&self) -> Option<&str> {
        self.capability_fingerprint.as_deref()
    }

    /// Query-policy fingerprint attached to the payload when present.
    pub fn query_policy_fingerprint(&self) -> Option<&str> {
        self.query_policy_fingerprint.as_deref()
    }

    /// Number of target-safe examples in a reflect request.
    pub const fn reflective_example_count(&self) -> usize {
        self.reflective_example_count
    }

    /// Proposal effects allowed by a propose request.
    pub fn allowed_effects(&self) -> &[StageProposalEffect] {
        &self.allowed_effects
    }

    /// Number of allowed change schemas declared by a propose request.
    pub const fn allowed_change_schema_count(&self) -> usize {
        self.allowed_change_schema_count
    }

    /// Number of output records carried by scorer or judge contexts.
    pub const fn output_count(&self) -> usize {
        self.output_count
    }

    /// Payload schema fingerprint for callback and adapter payloads.
    pub fn payload_schema(&self) -> Option<&str> {
        self.payload_schema.as_deref()
    }
}

/// Public-seam stage payload role.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StagePayloadRole {
    /// Reflector request.
    Reflector,
    /// Reflection result.
    ReflectionResult,
    /// Proposer request.
    Proposer,
    /// Runner request.
    Runner,
    /// Scorer context.
    Scorer,
    /// Judge context.
    Judge,
    /// Callback request.
    Callback,
    /// Artifact adapter request.
    ArtifactAdapter,
    /// Dataset adapter request.
    DatasetAdapter,
}

impl StagePayloadRole {
    fn parse(value: &str) -> Result<Self, PublicSeamError> {
        match value {
            "reflector" => Ok(Self::Reflector),
            "reflection_result" => Ok(Self::ReflectionResult),
            "proposer" => Ok(Self::Proposer),
            "runner" => Ok(Self::Runner),
            "scorer" => Ok(Self::Scorer),
            "judge" => Ok(Self::Judge),
            "callback" => Ok(Self::Callback),
            "artifact_adapter" => Ok(Self::ArtifactAdapter),
            "dataset_adapter" => Ok(Self::DatasetAdapter),
            other => Err(invalid_stage_payload(format!(
                "unknown stage payload role `{other}`"
            ))),
        }
    }
}

/// Proposal effects allowed by a public-seam propose request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StageProposalEffect {
    /// Create a fresh candidate.
    Create,
    /// Change an existing candidate.
    Change,
    /// Change an existing candidate from a workspace diff.
    ChangeFromWorkspaceDiff,
    /// Change an existing candidate from an agent session.
    ChangeFromAgentSession,
}

impl StageProposalEffect {
    fn parse(value: &str) -> Result<Self, PublicSeamError> {
        match value {
            "create" => Ok(Self::Create),
            "change" => Ok(Self::Change),
            "change_from_workspace_diff" => Ok(Self::ChangeFromWorkspaceDiff),
            "change_from_agent_session" => Ok(Self::ChangeFromAgentSession),
            other => Err(invalid_stage_payload(format!(
                "unknown proposal effect `{other}`"
            ))),
        }
    }

    const fn requires_change_schema(self) -> bool {
        matches!(
            self,
            Self::Change | Self::ChangeFromWorkspaceDiff | Self::ChangeFromAgentSession
        )
    }
}

fn inspect_reflect_request(
    object: &serde_json::Map<String, Value>,
) -> Result<usize, PublicSeamError> {
    require_field(object, "capability_fingerprint")?;
    require_field(object, "query_policy_fingerprint")?;
    if required_string(object.get("target_safety"), "target_safety")? != "target_safe_projection" {
        return Err(invalid_stage_payload(
            "reflector request must declare target_safe_projection",
        ));
    }
    require_non_empty_array(object.get("source_refs"), "source_refs")?;
    let examples = required_array(object.get("examples"), "examples")?;
    for example in examples {
        let example = example
            .as_object()
            .ok_or_else(|| invalid_stage_payload("reflective examples must be objects"))?;
        require_non_empty_array(example.get("source_refs"), "examples.source_refs")?;
        reject_target_leakage(example.get("input"), "reflector example input")?;
        reject_target_leakage(example.get("output"), "reflector example output")?;
        reject_target_leakage(example.get("feedback"), "reflector example feedback")?;
        reject_target_leakage(example.get("side_info"), "reflector example side_info")?;
        reject_target_leakage(example.get("score"), "reflector example score")?;
        for data_class in string_array(example.get("data_classes"), "examples.data_classes")? {
            if data_class == "case.target" {
                return Err(invalid_stage_payload(
                    "reflector examples must not carry case.target data classes",
                ));
            }
        }
    }
    Ok(examples.len())
}

fn inspect_reflection_result(
    object: &serde_json::Map<String, Value>,
    source_ref_count: usize,
    read_receipt_count: usize,
    data_classes: &[String],
) -> Result<(), PublicSeamError> {
    if required_string(object.get("summary"), "summary")?
        .trim()
        .is_empty()
    {
        return Err(invalid_stage_payload(
            "reflection_result summary must be non-empty",
        ));
    }
    if source_ref_count == 0 {
        return Err(invalid_stage_payload(
            "reflection_result must carry source refs",
        ));
    }
    if read_receipt_count == 0 {
        return Err(invalid_stage_payload(
            "reflection_result must carry read receipts",
        ));
    }
    if data_classes.is_empty() {
        return Err(invalid_stage_payload(
            "reflection_result must carry data classes",
        ));
    }
    Ok(())
}

fn inspect_propose_request(
    object: &serde_json::Map<String, Value>,
) -> Result<(Vec<StageProposalEffect>, usize), PublicSeamError> {
    require_field(object, "capability_fingerprint")?;
    require_field(object, "query_policy_fingerprint")?;
    require_non_empty_array(object.get("source_refs"), "source_refs")?;
    let reflection = object
        .get("reflection_result")
        .ok_or_else(|| invalid_stage_payload("propose request must carry ReflectionResult"))?;
    let reflection = StagePayloadDocument::from_schema_valid_value(reflection)?;
    if reflection.role() != StagePayloadRole::ReflectionResult {
        return Err(invalid_stage_payload(
            "propose request must consume a ReflectionResult payload",
        ));
    }
    let effects = required_array(object.get("allowed_effects"), "allowed_effects")?
        .iter()
        .map(|effect| {
            required_string(Some(effect), "allowed_effects").and_then(StageProposalEffect::parse)
        })
        .collect::<Result<Vec<_>, _>>()?;
    if effects.is_empty() {
        return Err(invalid_stage_payload(
            "propose request must declare allowed effects",
        ));
    }
    let change_schema_count = array_len(
        object.get("allowed_change_schemas"),
        "allowed_change_schemas",
    )?;
    if effects.iter().any(|effect| effect.requires_change_schema()) && change_schema_count == 0 {
        return Err(invalid_stage_payload(
            "change proposal effects must declare allowed_change_schemas",
        ));
    }
    Ok((effects, change_schema_count))
}

fn inspect_runner_request(object: &serde_json::Map<String, Value>) -> Result<(), PublicSeamError> {
    if object.get("target_forbidden") != Some(&Value::Bool(true)) {
        return Err(invalid_stage_payload(
            "runner request must declare target_forbidden=true",
        ));
    }
    reject_target_leakage(object.get("case_input"), "runner case_input")?;
    Ok(())
}

fn inspect_score_context(object: &serde_json::Map<String, Value>) -> Result<(), PublicSeamError> {
    require_field(object, "capability_fingerprint")?;
    require_field(object, "output")?;
    Ok(())
}

fn inspect_judge_context(object: &serde_json::Map<String, Value>) -> Result<(), PublicSeamError> {
    require_field(object, "capability_fingerprint")?;
    if array_len(object.get("outputs"), "outputs")? == 0 {
        return Err(invalid_stage_payload(
            "judge context must carry assessed outputs",
        ));
    }
    Ok(())
}

fn inspect_schema_bound_payload(
    object: &serde_json::Map<String, Value>,
) -> Result<(), PublicSeamError> {
    require_field(object, "capability_fingerprint")?;
    require_field(object, "payload_schema")?;
    Ok(())
}

fn require_field(
    object: &serde_json::Map<String, Value>,
    field: &str,
) -> Result<(), PublicSeamError> {
    object
        .get(field)
        .ok_or_else(|| invalid_stage_payload(format!("stage payload must carry `{field}`")))?;
    Ok(())
}

fn require_non_empty_array(value: Option<&Value>, field: &str) -> Result<(), PublicSeamError> {
    if required_array(value, field)?.is_empty() {
        return Err(invalid_stage_payload(format!(
            "stage payload field `{field}` must be non-empty"
        )));
    }
    Ok(())
}

fn reject_target_leakage(value: Option<&Value>, context: &str) -> Result<(), PublicSeamError> {
    let Some(value) = value else {
        return Ok(());
    };
    reject_target_leakage_value(value, context)
}

fn reject_target_leakage_value(value: &Value, context: &str) -> Result<(), PublicSeamError> {
    match value {
        Value::Object(object) => {
            for (key, nested) in object {
                if contains_case_target_marker(key) {
                    return Err(invalid_stage_payload(format!(
                        "{context} must not carry case.target material"
                    )));
                }
                reject_target_leakage_value(nested, context)?;
            }
        }
        Value::Array(items) => {
            for item in items {
                reject_target_leakage_value(item, context)?;
            }
        }
        Value::String(text) if contains_case_target_marker(text) => {
            return Err(invalid_stage_payload(format!(
                "{context} must not carry case.target material"
            )));
        }
        _ => {}
    }
    Ok(())
}

fn contains_case_target_marker(text: &str) -> bool {
    text.to_ascii_lowercase().contains("case.target")
}

fn required_string<'a>(value: Option<&'a Value>, field: &str) -> Result<&'a str, PublicSeamError> {
    value.and_then(Value::as_str).ok_or_else(|| {
        invalid_stage_payload(format!("stage payload field `{field}` must be a string"))
    })
}

fn optional_string(value: Option<&Value>) -> Result<Option<String>, PublicSeamError> {
    value
        .map(|value| {
            value
                .as_str()
                .map(ToOwned::to_owned)
                .ok_or_else(|| invalid_stage_payload("optional string field is not a string"))
        })
        .transpose()
}

fn required_array<'a>(
    value: Option<&'a Value>,
    field: &str,
) -> Result<&'a Vec<Value>, PublicSeamError> {
    value.and_then(Value::as_array).ok_or_else(|| {
        invalid_stage_payload(format!("stage payload field `{field}` must be an array"))
    })
}

fn array_len(value: Option<&Value>, field: &str) -> Result<usize, PublicSeamError> {
    value.map_or(Ok(0), |value| {
        value.as_array().map(Vec::len).ok_or_else(|| {
            invalid_stage_payload(format!("stage payload field `{field}` must be an array"))
        })
    })
}

fn string_array(value: Option<&Value>, field: &str) -> Result<Vec<String>, PublicSeamError> {
    value.map_or_else(
        || Ok(Vec::new()),
        |value| {
            value
                .as_array()
                .ok_or_else(|| {
                    invalid_stage_payload(format!("stage payload field `{field}` must be an array"))
                })?
                .iter()
                .map(|item| {
                    item.as_str().map(ToOwned::to_owned).ok_or_else(|| {
                        invalid_stage_payload(format!(
                            "stage payload field `{field}` must contain only strings"
                        ))
                    })
                })
                .collect()
        },
    )
}

fn invalid_stage_payload(message: impl Into<String>) -> PublicSeamError {
    PublicSeamError::InvalidStagePayload {
        message: message.into(),
    }
}
