//! Run-graph storage records.
//!
//! Mutators are deliberately `pub(crate)` — strategy authors write
//! through [`crate::context::RunContext`], not directly. The maps here
//! are the source of truth; [`super::indices::GraphIndices`] is
//! derived.
//!
//! This module currently defines the durable record shapes plus a
//! `RunGraph<P>` skeleton with the index maps allocated. Mutator
//! implementations land in subsequent passes alongside `RunContext`.

use indexmap::IndexMap;

use crate::artifact::{Artifact, ContentId};
use crate::candidate::CandidateOrigin;
use crate::error::ErrorRecord;
use crate::evaluation::{ResolvedEvaluationRequest, StoredAssessment};
use crate::ids::{
    ApplyAttemptId, AssessmentId, CandidateId, EvaluationRequestId, EvaluatorId, IterationId,
    ProposalBatchId, ProposalId, RunId, StageId,
};
use crate::metadata::MetadataBag;
use crate::population::PopulationEvent;
use crate::problem::OptimizationProblem;
use crate::proposal::{ProposalBatchSemantics, ProposalEffect, ProposalProvenance};
use crate::time::Timestamp;

use super::events::RunEvent;
use super::indices::GraphIndices;

#[derive(Clone, Debug)]
pub struct CandidateRecord<A: Artifact> {
    pub id: CandidateId,
    pub content_id: ContentId,
    pub artifact: A,
    pub origin: CandidateOrigin,
    pub created_at: Timestamp,
}

#[derive(Clone, Debug)]
pub struct ProposalBatchRecord {
    pub id: ProposalBatchId,
    pub stage: StageId,
    pub semantics: ProposalBatchSemantics,
    pub proposal_ids: Vec<ProposalId>,
    pub metadata: MetadataBag,
    pub created_at: Timestamp,
    pub iteration: Option<IterationId>,
}

#[derive(Clone, Debug)]
pub struct ProposalRecord<P: OptimizationProblem> {
    pub id: ProposalId,
    pub batch_id: ProposalBatchId,
    pub effect: ProposalEffect<P>,
    pub provenance: ProposalProvenance,
    pub annotations: P::ProposalAnnotations,
    pub metadata: MetadataBag,
    pub created_at: Timestamp,
}

#[derive(Clone, Debug)]
pub struct ApplyAttemptRecord {
    pub id: ApplyAttemptId,
    pub proposal_id: ProposalId,
    pub outcome: ApplyOutcome,
    pub created_at: Timestamp,
}

#[derive(Clone, Debug)]
pub enum ApplyOutcome {
    Created {
        candidate_id: CandidateId,
        content_id: ContentId,
    },
    Failed {
        error: ErrorRecord,
    },
}

#[derive(Clone, Debug)]
pub struct EvaluationRequestRecord {
    pub id: EvaluationRequestId,
    pub evaluator: EvaluatorId,
    pub request: ResolvedEvaluationRequest,
    pub created_at: Timestamp,
}

#[derive(Clone, Debug)]
pub struct AssessmentRecord {
    pub id: AssessmentId,
    pub request_id: EvaluationRequestId,
    pub evaluator: EvaluatorId,
    pub assessment: StoredAssessment,
    pub created_at: Timestamp,
}

#[derive(Clone, Debug)]
pub struct PopulationEventRecord {
    pub event: PopulationEvent,
    pub created_at: Timestamp,
}

#[derive(Clone, Debug)]
pub struct ErrorEventRecord {
    pub stage: Option<StageId>,
    pub error: ErrorRecord,
    pub created_at: Timestamp,
}

/// A budget-charged event recorded into the durable graph. Always
/// paired with a [`crate::graph::events::RunEvent::BudgetCharged`].
#[derive(Clone, Debug)]
pub struct BudgetEventRecord {
    pub stage: StageId,
    pub created_at: Timestamp,
}

#[derive(Clone, Debug)]
pub struct RunEventRecord<P: OptimizationProblem> {
    pub event: RunEvent<P>,
    pub created_at: Timestamp,
}

/// Source-of-truth run graph. Mutators are `pub(crate)`.
#[allow(dead_code)] // fields exercised once mutators land; stub today.
pub struct RunGraph<P: OptimizationProblem> {
    pub run_id: RunId,

    pub(crate) candidates: IndexMap<CandidateId, CandidateRecord<P::Artifact>>,
    pub(crate) proposal_batches: IndexMap<ProposalBatchId, ProposalBatchRecord>,
    pub(crate) proposals: IndexMap<ProposalId, ProposalRecord<P>>,
    pub(crate) apply_attempts: IndexMap<ApplyAttemptId, ApplyAttemptRecord>,

    pub(crate) evaluation_requests: IndexMap<EvaluationRequestId, EvaluationRequestRecord>,
    pub(crate) assessments: IndexMap<AssessmentId, AssessmentRecord>,

    pub(crate) population_events: Vec<PopulationEventRecord>,
    pub(crate) budget_events: Vec<BudgetEventRecord>,
    pub(crate) error_events: Vec<ErrorEventRecord>,
    pub(crate) events: Vec<RunEventRecord<P>>,

    pub(crate) indices: GraphIndices,
}

impl<P: OptimizationProblem> RunGraph<P> {
    #[must_use]
    pub fn new(run_id: RunId) -> Self {
        Self {
            run_id,
            candidates: IndexMap::new(),
            proposal_batches: IndexMap::new(),
            proposals: IndexMap::new(),
            apply_attempts: IndexMap::new(),
            evaluation_requests: IndexMap::new(),
            assessments: IndexMap::new(),
            population_events: Vec::new(),
            budget_events: Vec::new(),
            error_events: Vec::new(),
            events: Vec::new(),
            indices: GraphIndices::default(),
        }
    }
}
