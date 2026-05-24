use std::collections::BTreeSet;

use serde_json::Value;

use crate::PublicSeamError;

/// Schema-valid public-seam stage payload with role-specific semantic checks.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StagePayloadDocument {
    role: StagePayloadRole,
    source_ref_count: usize,
    read_receipt_count: usize,
    read_receipts: Vec<String>,
    data_classes: Vec<String>,
    capability_fingerprint: Option<String>,
    query_policy_fingerprint: Option<String>,
    reflective_example_count: usize,
    allowed_effects: Vec<StageProposalEffect>,
    allowed_change_schema_count: usize,
    output_count: usize,
    payload_schema: Option<String>,
}

/// Validated reflect-then-propose handoff across separate public-seam stages.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReflectProposeHandoffDocument {
    run: String,
    reflect_stage_call_id: String,
    propose_stage_call_id: String,
    base_revision: String,
    parent: String,
    surface_fingerprint: String,
    capability_fingerprint: String,
    query_policy_fingerprint: String,
    reflection_source_ref_count: usize,
}

impl ReflectProposeHandoffDocument {
    pub(crate) fn from_schema_valid_values(
        reflect_request: &Value,
        reflection_result: &Value,
        propose_request: &Value,
    ) -> Result<Self, PublicSeamError> {
        let reflect = StagePayloadDocument::from_schema_valid_value(reflect_request)?;
        let reflection = StagePayloadDocument::from_schema_valid_value(reflection_result)?;
        let propose = StagePayloadDocument::from_schema_valid_value(propose_request)?;
        if reflect.role() != StagePayloadRole::Reflector {
            return Err(invalid_stage_payload(
                "reflect/propose handoff must start with ReflectRequest",
            ));
        }
        if reflection.role() != StagePayloadRole::ReflectionResult {
            return Err(invalid_stage_payload(
                "reflect/propose handoff must carry ReflectionResult",
            ));
        }
        if propose.role() != StagePayloadRole::Proposer {
            return Err(invalid_stage_payload(
                "reflect/propose handoff must end with ProposeRequest",
            ));
        }

        let reflect = required_object(reflect_request, "reflect_request")?;
        let reflection = required_object(reflection_result, "reflection_result")?;
        let propose = required_object(propose_request, "propose_request")?;

        let embedded_reflection = propose.get("reflection_result").ok_or_else(|| {
            invalid_stage_payload("propose request must embed the consumed ReflectionResult")
        })?;
        if embedded_reflection != reflection_result {
            return Err(invalid_stage_payload(
                "propose request must consume the exact ReflectionResult in the handoff",
            ));
        }

        let run = matching_string(reflect, propose, "run")?;
        let base_revision = matching_string(reflect, propose, "base_revision")?;
        let surface_fingerprint = matching_string(reflect, propose, "surface_fingerprint")?;
        let capability_fingerprint = matching_string(reflect, propose, "capability_fingerprint")?;
        let query_policy_fingerprint =
            matching_string(reflect, propose, "query_policy_fingerprint")?;
        let parent = matching_source_ref(reflect, propose, "parent")?;

        let reflect_stage_call_id = required_string(
            reflect.get("stage_call_id"),
            "reflect_request.stage_call_id",
        )?
        .to_owned();
        let propose_stage_call_id = required_string(
            propose.get("stage_call_id"),
            "propose_request.stage_call_id",
        )?
        .to_owned();
        if reflect_stage_call_id == propose_stage_call_id {
            return Err(invalid_stage_payload(
                "reflect and propose stages must use distinct stage_call_id values",
            ));
        }

        let reflect_source_refs = source_ref_set(reflect.get("source_refs"), "source_refs")?;
        let reflection_source_refs = source_ref_set(
            reflection.get("source_refs"),
            "reflection_result.source_refs",
        )?;
        for source_ref in &reflection_source_refs {
            if !reflect_source_refs.contains(source_ref) {
                return Err(invalid_stage_payload(format!(
                    "reflect request source_refs must cover reflection source ref `{source_ref}`"
                )));
            }
        }

        Ok(Self {
            run,
            reflect_stage_call_id,
            propose_stage_call_id,
            base_revision,
            parent,
            surface_fingerprint,
            capability_fingerprint,
            query_policy_fingerprint,
            reflection_source_ref_count: reflection_source_refs.len(),
        })
    }

    /// Run id shared by the reflect and propose stages.
    pub fn run(&self) -> &str {
        &self.run
    }

    /// Stage call id for the reflector stage.
    pub fn reflect_stage_call_id(&self) -> &str {
        &self.reflect_stage_call_id
    }

    /// Stage call id for the proposer stage.
    pub fn propose_stage_call_id(&self) -> &str {
        &self.propose_stage_call_id
    }

    /// Base graph revision shared by both stage requests.
    pub fn base_revision(&self) -> &str {
        &self.base_revision
    }

    /// Normalized parent candidate ref shared by both stage requests.
    pub fn parent(&self) -> &str {
        &self.parent
    }

    /// Surface fingerprint shared by both stage requests.
    pub fn surface_fingerprint(&self) -> &str {
        &self.surface_fingerprint
    }

    /// Capability fingerprint shared by both stage requests.
    pub fn capability_fingerprint(&self) -> &str {
        &self.capability_fingerprint
    }

    /// Query-policy fingerprint shared by both stage requests.
    pub fn query_policy_fingerprint(&self) -> &str {
        &self.query_policy_fingerprint
    }

    /// Number of source refs carried by the consumed `ReflectionResult`.
    pub const fn reflection_source_ref_count(&self) -> usize {
        self.reflection_source_ref_count
    }
}

impl StagePayloadDocument {
    pub(crate) fn from_schema_valid_value(value: &Value) -> Result<Self, PublicSeamError> {
        let object = value
            .as_object()
            .ok_or_else(|| invalid_stage_payload("stage payload must be an object"))?;
        let role = StagePayloadRole::parse(required_string(object.get("role"), "role")?)?;
        let source_ref_count = array_len(object.get("source_refs"), "source_refs")?;
        let read_receipts = receipt_ref_ids(object.get("read_receipts"), "read_receipts")?;
        let read_receipt_count = read_receipts.len();
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
            read_receipts,
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

    /// Normalized read receipt refs carried by the payload.
    pub fn read_receipts(&self) -> &[String] {
        &self.read_receipts
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
    require_parent_source_ref(object)?;
    require_reflect_surface_context(object)?;
    require_non_empty_array(object.get("source_refs"), "source_refs")?;
    let top_level_source_refs = source_ref_set(object.get("source_refs"), "source_refs")?;
    let examples = required_array(object.get("examples"), "examples")?;
    if examples.is_empty() {
        return Err(invalid_stage_payload(
            "reflector request must carry target-safe examples",
        ));
    }
    for example in examples {
        let example = example
            .as_object()
            .ok_or_else(|| invalid_stage_payload("reflective examples must be objects"))?;
        require_non_empty_array(example.get("source_refs"), "examples.source_refs")?;
        require_source_ref_coverage(
            &top_level_source_refs,
            example.get("source_refs"),
            "reflector request source_refs",
            "reflective example source ref",
        )?;
        reject_target_leakage(example.get("input"), "reflector example input")?;
        reject_target_leakage(example.get("output"), "reflector example output")?;
        reject_target_leakage(example.get("feedback"), "reflector example feedback")?;
        reject_target_leakage(example.get("side_info"), "reflector example side_info")?;
        reject_target_leakage(example.get("score"), "reflector example score")?;
        let example_data_classes =
            string_array(example.get("data_classes"), "examples.data_classes")?;
        if example_data_classes.is_empty() {
            return Err(invalid_stage_payload(
                "reflector examples must carry data classes",
            ));
        }
        require_output_record_data_class_coverage(
            &example_data_classes,
            example
                .get("score")
                .and_then(|score| score.as_object())
                .and_then(|score| score.get("output")),
            "reflector example data_classes",
            "score output data class",
        )?;
        for data_class in example_data_classes {
            if contains_case_target_marker(&data_class) {
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
    require_read_receipt_refs(object.get("read_receipts"), "read_receipts")?;
    if data_classes.is_empty() {
        return Err(invalid_stage_payload(
            "reflection_result must carry data classes",
        ));
    }
    validate_reflection_diagnosis_sources(object)?;
    Ok(())
}

fn inspect_propose_request(
    object: &serde_json::Map<String, Value>,
) -> Result<(Vec<StageProposalEffect>, usize), PublicSeamError> {
    require_field(object, "capability_fingerprint")?;
    require_field(object, "query_policy_fingerprint")?;
    require_field(object, "surface_fingerprint")?;
    require_parent_source_ref(object)?;
    require_non_empty_array(object.get("source_refs"), "source_refs")?;
    let reflection_value = object
        .get("reflection_result")
        .ok_or_else(|| invalid_stage_payload("propose request must carry ReflectionResult"))?;
    let reflection = StagePayloadDocument::from_schema_valid_value(reflection_value)?;
    if reflection.role() != StagePayloadRole::ReflectionResult {
        return Err(invalid_stage_payload(
            "propose request must consume a ReflectionResult payload",
        ));
    }
    let reflection_source_refs = source_ref_set(
        reflection_value
            .as_object()
            .and_then(|object| object.get("source_refs")),
        "reflection_result.source_refs",
    )?;
    require_reflection_source_refs(object, reflection_source_refs)?;
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

fn require_parent_source_ref(
    object: &serde_json::Map<String, Value>,
) -> Result<(), PublicSeamError> {
    let parent = object
        .get("parent")
        .ok_or_else(|| invalid_stage_payload("stage payload must carry `parent`"))?;
    let parent_ref = source_ref_key(parent)?;
    let source_refs = source_ref_set(object.get("source_refs"), "source_refs")?;
    if !source_refs.contains(&parent_ref) {
        return Err(invalid_stage_payload(
            "stage payload source_refs must include the parent candidate",
        ));
    }
    Ok(())
}

fn require_reflect_surface_context(
    object: &serde_json::Map<String, Value>,
) -> Result<(), PublicSeamError> {
    require_field(object, "surface_fingerprint")?;
    if object.get("part").is_none() && object.get("part_label").is_none() {
        return Err(invalid_stage_payload(
            "reflector request must carry part or part_label context",
        ));
    }
    Ok(())
}

fn validate_reflection_diagnosis_sources(
    object: &serde_json::Map<String, Value>,
) -> Result<(), PublicSeamError> {
    let failure_modes = array_len(object.get("failure_modes"), "failure_modes")?;
    let surface_suggestions = array_len(object.get("surface_suggestions"), "surface_suggestions")?;
    if failure_modes + surface_suggestions == 0 {
        return Err(invalid_stage_payload(
            "reflection_result must carry source-backed diagnosis",
        ));
    }
    let top_level_source_refs = source_ref_set(object.get("source_refs"), "source_refs")?;
    require_nested_source_refs(
        &top_level_source_refs,
        object.get("failure_modes"),
        "failure_modes",
    )?;
    require_nested_source_refs(
        &top_level_source_refs,
        object.get("surface_suggestions"),
        "surface_suggestions",
    )?;
    Ok(())
}

fn require_nested_source_refs(
    top_level_source_refs: &BTreeSet<String>,
    value: Option<&Value>,
    field: &str,
) -> Result<(), PublicSeamError> {
    let Some(value) = value else {
        return Ok(());
    };
    let items = value.as_array().ok_or_else(|| {
        invalid_stage_payload(format!("stage payload field `{field}` must be an array"))
    })?;
    for item in items {
        let item = item.as_object().ok_or_else(|| {
            invalid_stage_payload(format!(
                "stage payload field `{field}` entries must be objects"
            ))
        })?;
        require_non_empty_array(item.get("source_refs"), &format!("{field}.source_refs"))?;
        require_source_ref_coverage(
            top_level_source_refs,
            item.get("source_refs"),
            "reflection_result source_refs",
            &format!("{field} source ref"),
        )?;
    }
    Ok(())
}

fn require_source_ref_coverage(
    top_level_source_refs: &BTreeSet<String>,
    nested_source_refs: Option<&Value>,
    top_level_field: &str,
    nested_label: &str,
) -> Result<(), PublicSeamError> {
    for source_ref in source_ref_set(nested_source_refs, nested_label)? {
        if !top_level_source_refs.contains(&source_ref) {
            return Err(invalid_stage_payload(format!(
                "{top_level_field} must cover {nested_label} `{source_ref}`"
            )));
        }
    }
    Ok(())
}

fn require_reflection_source_refs(
    propose: &serde_json::Map<String, Value>,
    reflection_source_refs: BTreeSet<String>,
) -> Result<(), PublicSeamError> {
    let propose_source_refs = source_ref_set(propose.get("source_refs"), "source_refs")?;
    for source_ref in reflection_source_refs {
        if !propose_source_refs.contains(&source_ref) {
            return Err(invalid_stage_payload(format!(
                "propose request source_refs must preserve reflection source ref `{source_ref}`"
            )));
        }
    }
    Ok(())
}

fn require_output_record_data_class_coverage(
    carrier: &[String],
    value: Option<&Value>,
    top_level_field: &str,
    nested_label: &str,
) -> Result<(), PublicSeamError> {
    for data_class in collect_output_record_data_classes(value, nested_label)? {
        if !carrier.contains(&data_class) {
            return Err(invalid_stage_payload(format!(
                "{top_level_field} must cover {nested_label} `{data_class}`"
            )));
        }
    }
    Ok(())
}

fn collect_output_record_data_classes(
    value: Option<&Value>,
    field: &str,
) -> Result<BTreeSet<String>, PublicSeamError> {
    let mut data_classes = BTreeSet::new();
    let Some(value) = value else {
        return Ok(data_classes);
    };
    let output = value
        .as_object()
        .ok_or_else(|| invalid_stage_payload(format!("{field} must be an object")))?;
    if let Some(classes) = output.get("data_classes") {
        data_classes.extend(string_array(Some(classes), field)?);
    }
    if let Some(blob_ref) = output.get("blob_ref") {
        let blob_ref = blob_ref
            .as_object()
            .ok_or_else(|| invalid_stage_payload(format!("{field} blob_ref must be an object")))?;
        if let Some(classes) = blob_ref.get("data_classes") {
            data_classes.extend(string_array(Some(classes), field)?);
        }
    }
    if let Some(trace_refs) = output.get("trace_refs") {
        let trace_refs = trace_refs
            .as_array()
            .ok_or_else(|| invalid_stage_payload(format!("{field} trace_refs must be an array")))?;
        for trace in trace_refs {
            let trace = trace.as_object().ok_or_else(|| {
                invalid_stage_payload(format!("{field} trace_refs entries must be objects"))
            })?;
            if let Some(classes) = trace.get("data_classes") {
                data_classes.extend(string_array(Some(classes), field)?);
            }
        }
    }
    Ok(data_classes)
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
    if let Some(target_handle) = object.get("target_handle") {
        let target_handle = required_string(Some(target_handle), "target_handle")?;
        let case = required_string(object.get("case"), "case")?;
        if target_handle != case {
            return Err(invalid_stage_payload(
                "score context target_handle must bind to the scored case",
            ));
        }
    }
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

fn required_object<'a>(
    value: &'a Value,
    field: &str,
) -> Result<&'a serde_json::Map<String, Value>, PublicSeamError> {
    value
        .as_object()
        .ok_or_else(|| invalid_stage_payload(format!("{field} must be an object")))
}

fn matching_string(
    left: &serde_json::Map<String, Value>,
    right: &serde_json::Map<String, Value>,
    field: &str,
) -> Result<String, PublicSeamError> {
    let left = required_string(left.get(field), field)?;
    let right = required_string(right.get(field), field)?;
    if left != right {
        return Err(invalid_stage_payload(format!(
            "reflect/propose handoff field `{field}` must match"
        )));
    }
    Ok(left.to_owned())
}

fn matching_source_ref(
    left: &serde_json::Map<String, Value>,
    right: &serde_json::Map<String, Value>,
    field: &str,
) -> Result<String, PublicSeamError> {
    let left = source_ref_key(
        left.get(field)
            .ok_or_else(|| invalid_stage_payload(format!("missing `{field}`")))?,
    )?;
    let right = source_ref_key(
        right
            .get(field)
            .ok_or_else(|| invalid_stage_payload(format!("missing `{field}`")))?,
    )?;
    if left != right {
        return Err(invalid_stage_payload(format!(
            "reflect/propose handoff field `{field}` must match"
        )));
    }
    Ok(left)
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

fn source_ref_set(value: Option<&Value>, field: &str) -> Result<BTreeSet<String>, PublicSeamError> {
    let Some(value) = value else {
        return Ok(BTreeSet::new());
    };
    let values = value.as_array().ok_or_else(|| {
        invalid_stage_payload(format!("stage payload field `{field}` must be an array"))
    })?;
    values
        .iter()
        .map(source_ref_key)
        .collect::<Result<BTreeSet<_>, _>>()
}

fn source_ref_key(value: &Value) -> Result<String, PublicSeamError> {
    if let Some(candidate) = candidate_ref_key(value)? {
        return Ok(candidate);
    }
    jcs_canonicalize::sha256_jcs_hex(value).map_err(|error| {
        invalid_stage_payload(format!(
            "stage payload source ref is not JCS canonicalizable: {error}"
        ))
    })
}

fn candidate_ref_key(value: &Value) -> Result<Option<String>, PublicSeamError> {
    if let Some(candidate) = value
        .as_str()
        .filter(|candidate| candidate.starts_with("cand_"))
    {
        return Ok(Some(format!("candidate:{candidate}")));
    }
    let Some(object) = value.as_object() else {
        return Ok(None);
    };
    if object.get("kind").and_then(Value::as_str) != Some("candidate") {
        return Ok(None);
    }
    let id = required_string(object.get("id"), "candidate ref id")?;
    Ok(Some(format!("candidate:{id}")))
}

fn receipt_ref_ids(value: Option<&Value>, field: &str) -> Result<Vec<String>, PublicSeamError> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let values = value.as_array().ok_or_else(|| {
        invalid_stage_payload(format!("stage payload field `{field}` must be an array"))
    })?;
    values
        .iter()
        .map(|value| receipt_ref_id(value, field))
        .collect()
}

fn require_read_receipt_refs(value: Option<&Value>, field: &str) -> Result<(), PublicSeamError> {
    for receipt in receipt_ref_ids(value, field)? {
        if !is_read_receipt_id(&receipt) {
            return Err(invalid_stage_payload(format!(
                "stage payload field `{field}` must contain read receipt refs, got `{receipt}`"
            )));
        }
    }
    Ok(())
}

fn receipt_ref_id(value: &Value, field: &str) -> Result<String, PublicSeamError> {
    if let Some(id) = value.as_str() {
        return Ok(id.to_owned());
    }
    let object = value
        .as_object()
        .ok_or_else(|| invalid_stage_payload(format!("{field} entries must be receipt refs")))?;
    if object.get("kind").and_then(Value::as_str) != Some("receipt") {
        return Err(invalid_stage_payload(format!(
            "{field} receipt ref object must have kind `receipt`"
        )));
    }
    object
        .get("id")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .ok_or_else(|| invalid_stage_payload(format!("{field} receipt ref object must carry id")))
}

fn is_read_receipt_id(receipt: &str) -> bool {
    receipt.starts_with("qrec_")
        || receipt.starts_with("caseread_")
        || receipt.starts_with("wsread_")
}

fn invalid_stage_payload(message: impl Into<String>) -> PublicSeamError {
    PublicSeamError::InvalidStagePayload {
        message: message.into(),
    }
}
