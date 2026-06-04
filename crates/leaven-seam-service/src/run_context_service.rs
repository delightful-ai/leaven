use std::{convert::Infallible, fmt};

use leaven_core::{
    Artifact, ArtifactIdentity, Assessment, AssessmentGranularity, AssessmentTarget, CacheIdentity,
    EvaluationPurpose, EvaluationRequest, EvaluationSet, Evidence, OptimizationProblem, Proposal,
    ProposalBatch, ProposalBatchSemantics,
};
use leaven_engine::{
    ApplyOutcome, BudgetLedger, CaseSet, ProposalBatchReport, RunContext, RunEvent, RunGraph,
};
use leaven_kernel::{
    Budget, CandidateId, ContentId, Cost, EvaluationRequestId, EvaluatorId, Fingerprint,
    MetadataBag, Metered, RunId, StageId,
};
use leaven_public_seam::{
    LockedMethod, PlanApplyProposalBatchRequest, PlanGraphQueryOutcome, PlanGraphQueryRequest,
    PublicSeamError,
};
use leaven_run::{
    PublicAssessmentWriteReceiptContext, PublicEvaluationJobContext,
    PublicProposalWriteReceiptContext,
};
use leaven_store_inline::InlineEvidenceStore;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::service::{SeamExecutionContextConfig, extension_result_for_plan_report};

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SeamRunContextConfig {
    /// Enables the RunContext-backed graph-write proof path.
    pub enabled: bool,
    /// Initial integer value inserted as the seed candidate.
    pub seed_value: i32,
    /// Integer delta carried by the staged mutation proposal.
    pub proposal_delta: i32,
    /// Public proposal-batch reference accepted by `leaven/proposal.apply`.
    pub proposal_batch_alias: String,
    /// Final graph revision projected after a successful apply.
    pub final_revision: String,
    /// Plan id routed to the RunContext graph readback summary.
    pub readback_plan_id: String,
}

impl Default for SeamRunContextConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            seed_value: 1,
            proposal_delta: 41,
            proposal_batch_alias: "pb_configured_run_context".to_owned(),
            final_revision: "rev_configured_run_context_applied".to_owned(),
            readback_plan_id: "runcontextgraphreadbackcli001".to_owned(),
        }
    }
}

pub(crate) struct RunContextProposalApplyState {
    graph: RunGraph<SeamTextProblem>,
    budget: BudgetLedger,
    case_set: CaseSet<()>,
    evidence_store: InlineEvidenceStore<SeamTextEvidence>,
    seed_candidate: CandidateId,
    batch: ProposalBatchReport,
    config: SeamRunContextConfig,
    created_candidates: Vec<String>,
    created_candidate_ids: Vec<CandidateId>,
    evaluation_request: Option<EvaluationRequestId>,
    evaluation_request_ref: Option<String>,
    assessment_ids: Vec<String>,
    emitted_events: Vec<Value>,
    event_count: usize,
    candidate_count: usize,
    applied: bool,
}

impl fmt::Debug for RunContextProposalApplyState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RunContextProposalApplyState")
            .field("batch", &self.batch)
            .field("config", &self.config)
            .field("created_candidates", &self.created_candidates)
            .field("evaluation_request_ref", &self.evaluation_request_ref)
            .field("assessment_ids", &self.assessment_ids)
            .field("emitted_events", &self.emitted_events)
            .field("event_count", &self.event_count)
            .field("candidate_count", &self.candidate_count)
            .field("applied", &self.applied)
            .finish_non_exhaustive()
    }
}

impl RunContextProposalApplyState {
    pub(crate) fn new(config: SeamRunContextConfig) -> Result<Self, PublicSeamError> {
        let mut graph = RunGraph::new(RunId::new());
        let mut budget = BudgetLedger::new(Budget::unlimited());
        let mut context = RunContext::<SeamTextProblem>::new(&mut graph, &mut budget);
        let seed = context
            .insert_seed(SeamTextArtifact(config.seed_value), 0)
            .map_err(invalid_run_context)?;
        let proposal = Proposal::mutate(seed, config.proposal_delta).build();
        let batch = context
            .record_proposal_batch(
                StageId::custom("seam-service-run-context"),
                ProposalBatch {
                    proposals: vec![proposal],
                    semantics: ProposalBatchSemantics::Alternatives,
                    metadata: MetadataBag::new(),
                },
                Cost::zero(),
            )
            .map_err(invalid_run_context)?;
        drop(context);
        Ok(Self {
            graph,
            budget,
            case_set: CaseSet::new(vec![()]),
            evidence_store: InlineEvidenceStore::new("seam-run-context"),
            seed_candidate: seed,
            batch,
            config,
            created_candidates: Vec::new(),
            created_candidate_ids: Vec::new(),
            evaluation_request: None,
            evaluation_request_ref: None,
            assessment_ids: Vec::new(),
            emitted_events: Vec::new(),
            event_count: 2,
            candidate_count: 1,
            applied: false,
        })
    }

    pub(crate) fn accepts_proposal_apply(&self, params: &Value) -> bool {
        proposal_apply_batch_ref(params) == Some(self.config.proposal_batch_alias.as_str())
    }

    pub(crate) fn accepts_proposal_batch_ref(&self, batch_ref: &str) -> bool {
        batch_ref == self.config.proposal_batch_alias
    }

    pub(crate) fn accepts_event_emit(&self, params: &Value) -> bool {
        event_emit_write(params)
            .map(|event| event.event_kind == "run_context.checked")
            .unwrap_or(false)
    }

    pub(crate) fn accepts_evaluation_request(&self, params: &Value) -> bool {
        request_evaluation_write(params)
            .map(|write| write.evaluator == "eval_run_context")
            .unwrap_or(false)
    }

    pub(crate) fn accepts_assessment_submit(&self, params: &Value) -> bool {
        let Some(expected) = self.evaluation_request_ref.as_deref() else {
            return false;
        };
        submit_assessments_write(params)
            .map(|write| {
                write.evaluation_request_id == expected
                    || write.evaluation_request_id == "evalreq_run_context_latest"
            })
            .unwrap_or(false)
    }

    pub(crate) fn accepts_graph_query_plan_id(&self, expr: &Value) -> bool {
        expr.get("source")
            .and_then(|source| source.get("filter"))
            .and_then(|filter| filter.get("kind"))
            .and_then(Value::as_str)
            == Some("run_context")
            || expr
                .get("source")
                .and_then(|source| source.get("kind"))
                .and_then(Value::as_str)
                == Some("run_context")
            || expr.get("plan_id").and_then(Value::as_str)
                == Some(self.config.readback_plan_id.as_str())
    }

    pub(crate) fn apply_proposal_batch(
        &mut self,
        method: LockedMethod,
        params: &Value,
        context: &SeamExecutionContextConfig,
    ) -> Result<Value, PublicSeamError> {
        let batch_ref =
            proposal_apply_batch_ref(params).ok_or_else(|| PublicSeamError::InvalidPlan {
                message: "RunContext proposal.apply requires proposal_batch".to_owned(),
            })?;
        if batch_ref != self.config.proposal_batch_alias {
            return Err(PublicSeamError::InvalidPlan {
                message: format!(
                    "RunContext proposal.apply cannot satisfy proposal batch `{batch_ref}`"
                ),
            });
        }
        let mut run_context = RunContext::<SeamTextProblem>::new(&mut self.graph, &mut self.budget);
        let apply = run_context
            .apply_batch(self.batch.batch_id)
            .map_err(invalid_run_context)?;
        let graph = run_context.graph();
        let plan_id = params
            .get("plan_id")
            .and_then(Value::as_str)
            .unwrap_or("runcontextproposalapply001");
        let plan_result = PublicProposalWriteReceiptContext::new(
            plan_id,
            &context.base_revision,
            &self.config.final_revision,
            &context.capability_fingerprint,
            &context.policy_fingerprint,
        )
        .with_submit_timing(&context.started_at, &context.started_at)
        .with_apply_timing(&context.started_at, &context.completed_at)
        .proposal_apply_plan_result(&graph, &self.batch, &apply)
        .map_err(|error| PublicSeamError::InvalidPlan {
            message: format!("RunContext proposal.apply projection failed: {error}"),
        })?;
        self.created_candidates = apply
            .outcomes
            .iter()
            .filter_map(|outcome| match outcome.outcome {
                ApplyOutcome::Success { candidate_id } => {
                    Some(format!("cand_{}", candidate_id.as_uuid()))
                }
                ApplyOutcome::Failure { .. } => None,
            })
            .collect();
        self.created_candidate_ids = apply.successful_candidates().collect();
        self.candidate_count += self.created_candidates.len();
        self.applied = true;
        extension_result_for_plan_report(method, params, &plan_result)
    }

    pub(crate) fn request_evaluation(
        &mut self,
        method: LockedMethod,
        params: &Value,
        context: &SeamExecutionContextConfig,
    ) -> Result<Value, PublicSeamError> {
        request_evaluation_write(params)?;
        let candidates = if self.created_candidate_ids.is_empty() {
            vec![self.seed_candidate]
        } else {
            self.created_candidate_ids.clone()
        };
        let request = EvaluationRequest::Independent {
            candidates,
            set: EvaluationSet::All,
            granularity: AssessmentGranularity::PerCase,
            purpose: EvaluationPurpose::Validation,
        };
        let mut run_context = RunContext::<SeamTextProblem>::new(&mut self.graph, &mut self.budget)
            .with_case_set(&self.case_set);
        let request_id = run_context
            .request_evaluation(
                EvaluatorId::from("eval_run_context"),
                Fingerprint::from_bytes([57; 32]),
                request,
            )
            .map_err(invalid_run_context)?;
        let graph = run_context.graph();
        let request = graph
            .evaluation_request(request_id)
            .ok_or_else(|| invalid_plan("RunContext evaluation request was not readable"))?;
        let job_context = PublicEvaluationJobContext::new(
            "sc_run_context_evaluation_request",
            &context.base_revision,
            &context.capability_fingerprint,
            &context.policy_fingerprint,
            "2026-05-23T00:20:00Z",
        )
        .with_evaluation_request_receipt_timing(&context.started_at, &context.completed_at);
        let job = job_context
            .evaluation_job_document(&graph, &request)
            .map_err(|error| invalid_plan(format!("RunContext evaluation job failed: {error}")))?;
        let plan_result = job_context
            .evaluation_request_receipt_plan_result(&job)
            .map_err(|error| {
                invalid_plan(format!("RunContext evaluation receipt failed: {error}"))
            })?;
        self.evaluation_request = Some(request_id);
        self.evaluation_request_ref = Some(format!("evalreq_{}", request_id.as_uuid()));
        extension_result_for_plan_report(method, params, &plan_result)
    }

    pub(crate) fn submit_assessments(
        &mut self,
        method: LockedMethod,
        params: &Value,
        context: &SeamExecutionContextConfig,
    ) -> Result<Value, PublicSeamError> {
        submit_assessments_write(params)?;
        let request_id = self.evaluation_request.ok_or_else(|| {
            invalid_plan("RunContext assessment submit requires prior evaluation request")
        })?;
        let candidates = if self.created_candidate_ids.is_empty() {
            vec![self.seed_candidate]
        } else {
            self.created_candidate_ids.clone()
        };
        let assessments = candidates
            .into_iter()
            .map(|candidate| Assessment::Independent {
                candidate,
                target: AssessmentTarget::Unscoped,
                evidence: SeamTextEvidence,
                cost: Cost::zero(),
                metadata: MetadataBag::new(),
            })
            .collect::<Vec<_>>();
        let mut run_context = RunContext::<SeamTextProblem>::new(&mut self.graph, &mut self.budget)
            .with_evidence_store(&self.evidence_store);
        let report = run_context
            .submit_assessments(request_id, Metered::new(assessments, Cost::zero()))
            .map_err(invalid_run_context)?;
        let graph = run_context.graph();
        let plan_id = params
            .get("plan_id")
            .and_then(Value::as_str)
            .unwrap_or("runcontextassessmentsubmit001");
        let plan_result = PublicAssessmentWriteReceiptContext::new(
            plan_id,
            &context.base_revision,
            &self.config.final_revision,
            &context.capability_fingerprint,
            &context.policy_fingerprint,
        )
        .with_timing(&context.started_at, &context.completed_at)
        .submit_assessments_plan_result(&graph, &report)
        .map_err(|error| {
            invalid_plan(format!("RunContext assessment projection failed: {error}"))
        })?;
        self.assessment_ids = report
            .assessment_ids
            .iter()
            .map(|id| format!("assess_{}", id.as_uuid()))
            .collect();
        extension_result_for_plan_report(method, params, &plan_result)
    }

    pub(crate) fn emit_run_event(
        &mut self,
        method: LockedMethod,
        params: &Value,
        context: &SeamExecutionContextConfig,
    ) -> Result<Value, PublicSeamError> {
        let event = event_emit_write(params)?;
        let event_id = format!("event_{}", event.name);
        let mut run_context = RunContext::<SeamTextProblem>::new(&mut self.graph, &mut self.budget);
        run_context.emit(RunEvent::ExternalEventEmitted {
            event_id: event_id.clone(),
            event_kind: event.event_kind.to_owned(),
            payload_schema: event.payload_schema.to_owned(),
            payload: event.payload.clone(),
            visibility: event.visibility.to_owned(),
        });
        self.event_count = run_context.graph().events().count();
        self.emitted_events.push(json!({
            "event_id": event_id,
            "event_kind": event.event_kind,
            "payload_schema": event.payload_schema,
            "payload": event.payload,
            "visibility": event.visibility
        }));
        run_context_event_emit_extension_result(method, event, context)
    }

    pub(crate) fn graph_query(&self, request: PlanGraphQueryRequest<'_>) -> PlanGraphQueryOutcome {
        PlanGraphQueryOutcome::new(
            [json!({
                "kind": "event_summary",
                "event_kind": "proposal.apply",
                "revision": self.config.final_revision,
                "payload": {
                    "source": "leaven-seam-service-run-context",
                    "candidate_count": self.candidate_count,
                    "proposal_batch": self.config.proposal_batch_alias,
                    "created_candidates": self.created_candidates,
                    "event_count": self.event_count,
                    "emitted_events": self.emitted_events,
                    "evaluation_request_id": self.evaluation_request_ref,
                    "assessment_ids": self.assessment_ids,
                    "applied": self.applied
                }
            })],
            graph_query_revision(request, &self.config.final_revision),
        )
    }
}

pub(crate) fn requested_proposal_batch<'a>(
    request: &'a PlanApplyProposalBatchRequest<'a>,
) -> Result<&'a str, PublicSeamError> {
    request
        .write()
        .get("proposal_batch")
        .and_then(Value::as_str)
        .ok_or_else(|| PublicSeamError::InvalidPlan {
            message: "apply_proposal_batch must carry proposal_batch".to_owned(),
        })
}

fn proposal_apply_batch_ref(params: &Value) -> Option<&str> {
    params
        .get("ops")?
        .as_array()?
        .iter()
        .filter_map(|op| op.get("write"))
        .find(|write| write.get("kind").and_then(Value::as_str) == Some("apply_proposal_batch"))
        .and_then(|write| write.get("proposal_batch"))
        .and_then(Value::as_str)
}

struct RequestEvaluationWrite<'a> {
    evaluator: &'a str,
}

fn request_evaluation_write(params: &Value) -> Result<RequestEvaluationWrite<'_>, PublicSeamError> {
    let ops = params
        .get("ops")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid_plan("evaluation.request plan must carry ops"))?;
    for op in ops {
        let Some(write) = op.get("write") else {
            continue;
        };
        if write.get("kind").and_then(Value::as_str) == Some("request_evaluation") {
            let request = write
                .get("request")
                .ok_or_else(|| invalid_plan("request_evaluation must carry request"))?;
            return Ok(RequestEvaluationWrite {
                evaluator: request
                    .get("evaluator")
                    .and_then(Value::as_str)
                    .unwrap_or("eval_run_context"),
            });
        }
    }
    Err(invalid_plan(
        "evaluation.request method must carry a request_evaluation write",
    ))
}

struct SubmitAssessmentsWrite<'a> {
    evaluation_request_id: &'a str,
}

fn submit_assessments_write(params: &Value) -> Result<SubmitAssessmentsWrite<'_>, PublicSeamError> {
    let ops = params
        .get("ops")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid_plan("assessment.submit plan must carry ops"))?;
    for op in ops {
        let Some(write) = op.get("write") else {
            continue;
        };
        if write.get("kind").and_then(Value::as_str) == Some("submit_assessments") {
            return Ok(SubmitAssessmentsWrite {
                evaluation_request_id: write
                    .get("evaluation_request_id")
                    .and_then(Value::as_str)
                    .ok_or_else(|| {
                    invalid_plan("submit_assessments must carry evaluation_request_id")
                })?,
            });
        }
    }
    Err(invalid_plan(
        "assessment.submit method must carry a submit_assessments write",
    ))
}

struct EventEmitWrite<'a> {
    name: &'a str,
    write: &'a Value,
    event_kind: &'a str,
    payload_schema: &'a str,
    payload: &'a Value,
    visibility: &'a str,
}

fn event_emit_write(params: &Value) -> Result<EventEmitWrite<'_>, PublicSeamError> {
    let ops = params
        .get("ops")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid_plan("event.emit plan must carry ops"))?;
    for op in ops {
        let Some(write) = op.get("write") else {
            continue;
        };
        if write.get("kind").and_then(Value::as_str) == Some("emit_run_event") {
            return Ok(EventEmitWrite {
                name: string_field(op, "name")?,
                write,
                event_kind: string_field(write, "event_kind")?,
                payload_schema: string_field(write, "payload_schema")?,
                payload: write
                    .get("payload")
                    .ok_or_else(|| invalid_plan("emit_run_event must carry payload"))?,
                visibility: string_field(write, "visibility")?,
            });
        }
    }
    Err(invalid_plan(
        "event.emit method must carry an emit_run_event write",
    ))
}

fn run_context_event_emit_extension_result(
    method: LockedMethod,
    event: EventEmitWrite<'_>,
    context: &SeamExecutionContextConfig,
) -> Result<Value, PublicSeamError> {
    let event_id = format!("event_{}", event.name);
    let receipt_id = format!("wrec_{}", event.name);
    let request_hash = prefixed_jcs_hash(
        "fp_request_sha256_",
        &json!({
            "schema_version": "leaven.plan_write_request.v1",
            "name": event.name,
            "kind": "emit_run_event",
            "write": event.write,
            "deps": {},
            "dependency_data_classes": [],
            "base_revision": context.base_revision
        }),
    )?;
    let primary = json!({
        "kind": "emit_run_event",
        "event_id": event_id,
        "receipt": receipt_id,
        "data_classes": ["public"],
        "replayability": "fully_managed"
    });
    let result_hash = prefixed_jcs_hash(
        "fp_result_sha256_",
        &json!({
            "schema_version": "leaven.plan_write_result.v1",
            "name": event.name,
            "value": primary
        }),
    )?;
    Ok(json!({
        "method": method.as_str(),
        "primary": primary,
        "receipts": [{
            "kind": "write",
            "receipt": primary["receipt"],
            "op_var": event.name,
            "started_at": context.started_at,
            "completed_at": context.completed_at,
            "write_kind": "emit_run_event",
            "request_hash": request_hash,
            "result_hash": result_hash,
            "base_revision": context.base_revision,
            "committed_revision": context.base_revision,
            "status": "succeeded",
            "event_id": primary["event_id"]
        }],
        "redactions": [],
        "capability_fingerprint": context.capability_fingerprint,
        "policy_fingerprint": context.policy_fingerprint,
        "data_classes": ["public"]
    }))
}

fn string_field<'a>(value: &'a Value, field: &'static str) -> Result<&'a str, PublicSeamError> {
    value
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid_plan(format!("{field} must be a string")))
}

fn graph_query_revision(request: PlanGraphQueryRequest<'_>, default_revision: &str) -> String {
    match request.scope() {
        leaven_public_seam::PlanGraphReadScope::LatestAtStart { revision }
        | leaven_public_seam::PlanGraphReadScope::AtRevision { revision } => revision.to_owned(),
        leaven_public_seam::PlanGraphReadScope::SinceRevision { since: _, until } => {
            until.unwrap_or(default_revision).to_owned()
        }
    }
}

fn invalid_run_context(error: impl std::fmt::Display) -> PublicSeamError {
    PublicSeamError::InvalidPlan {
        message: format!("RunContext-backed seam service failed: {error}"),
    }
}

fn invalid_plan(message: impl Into<String>) -> PublicSeamError {
    PublicSeamError::InvalidPlan {
        message: message.into(),
    }
}

fn prefixed_jcs_hash(prefix: &str, value: &Value) -> Result<String, PublicSeamError> {
    let digest =
        jcs_canonicalize::sha256_jcs_hex(value).map_err(|error| PublicSeamError::InvalidPlan {
            message: format!("RunContext event.emit receipt hash failed: {error}"),
        })?;
    Ok(format!("{prefix}{digest}"))
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SeamTextArtifact(i32);

impl Artifact for SeamTextArtifact {
    type Change = i32;
    type ApplyError = Infallible;

    fn identity(&self) -> ArtifactIdentity {
        ArtifactIdentity::External(format!("seam-text-{}", self.0))
    }

    fn cache_identity(&self) -> Option<CacheIdentity> {
        Some(CacheIdentity::Content(ContentId::hash_bytes(
            self.0.to_string().as_bytes(),
        )))
    }

    fn apply_change(&self, change: &Self::Change) -> Result<Self, Self::ApplyError> {
        Ok(Self(self.0 + change))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SeamTextEvidence;

impl Evidence for SeamTextEvidence {}

struct SeamTextProblem;

impl OptimizationProblem for SeamTextProblem {
    type Artifact = SeamTextArtifact;
    type Case = ();
    type Evidence = SeamTextEvidence;
    type ProposalAnnotations = ();
}
