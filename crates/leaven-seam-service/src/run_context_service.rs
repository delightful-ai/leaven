use std::{convert::Infallible, fmt};

use leaven_core::{
    Artifact, ArtifactIdentity, CacheIdentity, Evidence, OptimizationProblem, Proposal,
    ProposalBatch, ProposalBatchSemantics,
};
use leaven_engine::{ApplyOutcome, BudgetLedger, ProposalBatchReport, RunContext, RunGraph};
use leaven_kernel::{Budget, ContentId, Cost, MetadataBag, RunId, StageId};
use leaven_public_seam::{
    PlanApplyProposalBatchRequest, PlanGraphQueryOutcome, PlanGraphQueryRequest, PublicSeamError,
};
use leaven_run::PublicProposalWriteReceiptContext;
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
    batch: ProposalBatchReport,
    config: SeamRunContextConfig,
    created_candidates: Vec<String>,
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
            batch,
            config,
            created_candidates: Vec::new(),
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
        method: &str,
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
        self.candidate_count += self.created_candidates.len();
        self.applied = true;
        extension_result_for_plan_report(method, params, &plan_result)
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
