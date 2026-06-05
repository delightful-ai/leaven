use std::collections::BTreeSet;

use serde_json::Value;

use crate::PublicSeamError;

mod inspect;
mod inspect_helpers;

use inspect::{
    inspect_judge_context, inspect_propose_request, inspect_reflect_request,
    inspect_reflection_result, inspect_runner_request, inspect_schema_bound_payload,
    inspect_score_context, validate_submit_proposal_batch_for_handoff,
};
use inspect_helpers::{
    array_len, invalid_stage_payload, matching_source_ref, matching_string, optional_string,
    prefixed_stage_payload_hash, receipt_ref_ids, required_array, required_object, required_string,
    source_ref_set, string_array, string_set, validate_handoff_stage_receipts,
};

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
    runner_case_input: Option<RunnerCaseInputDocument>,
}

/// Typed runner-stage case input facts.
///
/// The public seam validates the exact JSON object at ingress, but downstream
/// code should not need to re-walk raw JSON to know which candidate/case pair
/// a runner saw or to compare the case-input object by content.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RunnerCaseInputDocument {
    candidate: String,
    case: String,
    case_input: RunnerCaseInputValue,
    case_input_fingerprint: String,
    case_input_keys: Vec<String>,
}

/// Schema-valid target-free JSON object carried by a runner-stage case input.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RunnerCaseInputValue(Value);

/// Validated reflect-then-propose handoff across separate public-seam stages.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReflectProposeHandoffDocument {
    run: String,
    reflect_stage_call_id: String,
    propose_stage_call_id: String,
    reflect_stage_receipt: String,
    propose_stage_receipt: String,
    base_revision: String,
    parent: String,
    surface_fingerprint: String,
    capability_fingerprint: String,
    query_policy_fingerprint: String,
    reflection_result_fingerprint: String,
    reflection_source_ref_count: usize,
}

/// Validated proposal submission citing a separate reflect/propose handoff.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReflectProposeSubmissionDocument {
    handoff: ReflectProposeHandoffDocument,
    submit_batches: usize,
    proposal_count: usize,
    create_effects: usize,
    change_effects: usize,
    workspace_diff_effects: usize,
    agent_session_effects: usize,
    stage_provenance_links: usize,
}

impl ReflectProposeHandoffDocument {
    pub(crate) fn from_schema_valid_values(
        handoff: &Value,
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
        let reflection_result_fingerprint =
            prefixed_stage_payload_hash("fp_stage_payload_sha256_", reflection_result)?;
        let (reflect_stage_receipt, propose_stage_receipt) = validate_handoff_stage_receipts(
            handoff,
            &reflect_stage_call_id,
            &propose_stage_call_id,
            &reflection_result_fingerprint,
        )?;

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
            reflect_stage_receipt,
            propose_stage_receipt,
            base_revision,
            parent,
            surface_fingerprint,
            capability_fingerprint,
            query_policy_fingerprint,
            reflection_result_fingerprint,
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

    /// Stage receipt proving the reflector produced the consumed reflection result.
    pub fn reflect_stage_receipt(&self) -> &str {
        &self.reflect_stage_receipt
    }

    /// Stage receipt proving the proposer consumed the reflected diagnosis.
    pub fn propose_stage_receipt(&self) -> &str {
        &self.propose_stage_receipt
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

    /// JCS fingerprint of the exact consumed `ReflectionResult`.
    pub fn reflection_result_fingerprint(&self) -> &str {
        &self.reflection_result_fingerprint
    }

    /// Number of source refs carried by the consumed `ReflectionResult`.
    pub const fn reflection_source_ref_count(&self) -> usize {
        self.reflection_source_ref_count
    }
}

impl ReflectProposeSubmissionDocument {
    pub(crate) fn from_valid_handoff_and_plan(
        handoff: ReflectProposeHandoffDocument,
        handoff_value: &Value,
        plan: &Value,
    ) -> Result<Self, PublicSeamError> {
        let propose = handoff_value
            .pointer("/propose_request")
            .and_then(Value::as_object)
            .ok_or_else(|| {
                invalid_stage_payload("reflect/propose submission must carry /propose_request")
            })?;
        let allowed_effects = required_array(propose.get("allowed_effects"), "allowed_effects")?
            .iter()
            .map(|effect| {
                required_string(Some(effect), "allowed_effects")
                    .and_then(StageProposalEffect::parse)
            })
            .collect::<Result<BTreeSet<_>, _>>()?;
        let allowed_change_schemas = string_set(
            propose.get("allowed_change_schemas"),
            "allowed_change_schemas",
        )?;
        let reflection_read_receipts = receipt_ref_ids(
            propose
                .get("reflection_result")
                .and_then(|reflection| reflection.as_object())
                .and_then(|reflection| reflection.get("read_receipts")),
            "reflection_result.read_receipts",
        )?;

        let mut document = Self {
            handoff,
            submit_batches: 0,
            proposal_count: 0,
            create_effects: 0,
            change_effects: 0,
            workspace_diff_effects: 0,
            agent_session_effects: 0,
            stage_provenance_links: 0,
        };
        let ops = plan.get("ops").and_then(Value::as_array).ok_or_else(|| {
            invalid_stage_payload("proposal submission plan ops must be an array")
        })?;
        for op in ops {
            let Some(write) = op.get("write").and_then(Value::as_object) else {
                continue;
            };
            if required_string(write.get("kind"), "write.kind")? == "submit_proposal_batch" {
                document.submit_batches += 1;
                validate_submit_proposal_batch_for_handoff(
                    write,
                    &allowed_effects,
                    &allowed_change_schemas,
                    &reflection_read_receipts,
                    &mut document,
                )?;
            }
        }
        if document.submit_batches == 0 {
            return Err(invalid_stage_payload(
                "reflect/propose submission must carry a submit_proposal_batch write",
            ));
        }
        Ok(document)
    }

    /// Validated reflect/propose handoff cited by this proposal submission.
    pub const fn handoff(&self) -> &ReflectProposeHandoffDocument {
        &self.handoff
    }

    /// Number of `submit_proposal_batch` writes checked.
    pub const fn submit_batches(&self) -> usize {
        self.submit_batches
    }

    /// Number of proposals checked.
    pub const fn proposal_count(&self) -> usize {
        self.proposal_count
    }

    /// Number of `create` proposal effects checked.
    pub const fn create_effects(&self) -> usize {
        self.create_effects
    }

    /// Number of `change` proposal effects checked.
    pub const fn change_effects(&self) -> usize {
        self.change_effects
    }

    /// Number of `change_from_workspace_diff` proposal effects checked.
    pub const fn workspace_diff_effects(&self) -> usize {
        self.workspace_diff_effects
    }

    /// Number of `change_from_agent_session` proposal effects checked.
    pub const fn agent_session_effects(&self) -> usize {
        self.agent_session_effects
    }

    /// Number of proposals that cite the proposer stage receipt in `informed_by`.
    pub const fn stage_provenance_links(&self) -> usize {
        self.stage_provenance_links
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
        let runner_case_input = match role {
            StagePayloadRole::ReflectionResult => {
                inspect_reflection_result(
                    object,
                    source_ref_count,
                    read_receipt_count,
                    &data_classes,
                )?;
                None
            }
            StagePayloadRole::Runner => Some(inspect_runner_request(object)?),
            StagePayloadRole::Scorer => {
                inspect_score_context(object)?;
                None
            }
            StagePayloadRole::Judge => {
                inspect_judge_context(object)?;
                None
            }
            StagePayloadRole::Callback
            | StagePayloadRole::ArtifactAdapter
            | StagePayloadRole::DatasetAdapter => {
                inspect_schema_bound_payload(object)?;
                None
            }
            StagePayloadRole::Reflector | StagePayloadRole::Proposer => None,
        };
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
            runner_case_input,
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

    /// Runner-stage case-input facts when this payload is a runner request.
    pub const fn runner_case_input(&self) -> Option<&RunnerCaseInputDocument> {
        self.runner_case_input.as_ref()
    }
}

impl RunnerCaseInputDocument {
    pub(super) fn new(
        candidate: String,
        case: String,
        case_input: RunnerCaseInputValue,
        case_input_fingerprint: String,
        case_input_keys: Vec<String>,
    ) -> Self {
        Self {
            candidate,
            case,
            case_input,
            case_input_fingerprint,
            case_input_keys,
        }
    }

    /// Candidate id whose artifact the runner executed.
    pub fn candidate(&self) -> &str {
        &self.candidate
    }

    /// Case id whose target-free input the runner received.
    pub fn case(&self) -> &str {
        &self.case
    }

    /// Target-free JSON object received by the runner.
    pub const fn case_input(&self) -> &RunnerCaseInputValue {
        &self.case_input
    }

    /// Stable JCS SHA-256 fingerprint of the exact case-input object.
    pub fn case_input_fingerprint(&self) -> &str {
        &self.case_input_fingerprint
    }

    /// Sorted top-level keys present in the case-input object.
    pub fn case_input_keys(&self) -> &[String] {
        &self.case_input_keys
    }
}

impl RunnerCaseInputValue {
    pub(super) fn from_schema_valid_value(value: &Value) -> Self {
        Self(value.clone())
    }

    /// JSON object carried on the wire by the runner case input.
    pub const fn as_json(&self) -> &Value {
        &self.0
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

    /// Wire spelling of the stage payload role.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Reflector => "reflector",
            Self::ReflectionResult => "reflection_result",
            Self::Proposer => "proposer",
            Self::Runner => "runner",
            Self::Scorer => "scorer",
            Self::Judge => "judge",
            Self::Callback => "callback",
            Self::ArtifactAdapter => "artifact_adapter",
            Self::DatasetAdapter => "dataset_adapter",
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

    const fn as_str(self) -> &'static str {
        match self {
            Self::Create => "create",
            Self::Change => "change",
            Self::ChangeFromWorkspaceDiff => "change_from_workspace_diff",
            Self::ChangeFromAgentSession => "change_from_agent_session",
        }
    }
}

impl Ord for StageProposalEffect {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.as_str().cmp(other.as_str())
    }
}

impl PartialOrd for StageProposalEffect {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
