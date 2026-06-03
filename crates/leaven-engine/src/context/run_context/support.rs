use leaven_core::{
    Artifact, Assessment, CacheIdentity, EvaluationRequest, OptimizationProblem,
    ResolvedEvaluationRequest, ResolvedRequestKind,
};
use leaven_kernel::{
    AssessmentId, BudgetExceeded, CandidateId, EvaluationRequestId, EvaluatorId, ProposalBatchId,
};
use leaven_store::StoreError;
use thiserror::Error;

use crate::graph::storage::{ApplyProposalError, AssessmentRecordTarget};
use crate::{
    CacheBypassReason, CachePolicy, EvaluationCacheKey, EvaluationError, EvaluationResolveError,
    ProposalError, RunGraphView, TrustViolation,
};

/// Failures from the public run-context mutation surface.
#[derive(Debug, Error)]
pub enum RunContextError {
    /// Graph insertion or proposal-application refusal.
    #[error(transparent)]
    Graph(#[from] ApplyProposalError),
    /// Evaluation-set resolution refused the request.
    #[error(transparent)]
    EvaluationResolve(#[from] EvaluationResolveError),
    /// The requested proposal batch is not present in the graph.
    #[error("unknown proposal batch: {0}")]
    UnknownBatch(ProposalBatchId),
    /// The requested evaluator is not registered in the engine.
    #[error("unknown evaluator: {0}")]
    UnknownEvaluator(EvaluatorId),
    /// The requested evaluation request is not visible or not present in the graph.
    #[error("unknown evaluation request: {0}")]
    UnknownEvaluationRequest(EvaluationRequestId),
    /// The requested assessment is not visible or not present in the graph.
    #[error("unknown assessment: {0}")]
    UnknownAssessment(AssessmentId),
    /// A budget ledger refused a charge.
    #[error(transparent)]
    Budget(#[from] BudgetExceeded),
    /// A proposer refused its request.
    #[error(transparent)]
    Proposal(#[from] ProposalError),
    /// An evaluator refused its request.
    #[error(transparent)]
    Evaluation(#[from] EvaluationError),
    /// Evidence or checkpoint storage refused an operation.
    #[error(transparent)]
    Store(#[from] StoreError),
    /// Run persistence refused a checkpoint.
    #[error(transparent)]
    Persistence(#[from] crate::RunPersistenceError),
    /// Trust policy refused a request.
    #[error(transparent)]
    TrustViolation(#[from] TrustViolation),
    /// Evaluation was requested without a case set.
    #[error("case set is required")]
    MissingCaseSet,
    /// Assessment evidence storage or retrieval was requested without a store.
    #[error("evidence store is required")]
    MissingEvidenceStore,
}

pub(super) fn resolved_kind(request: &EvaluationRequest) -> ResolvedRequestKind {
    match request {
        EvaluationRequest::Independent { candidates, .. } => ResolvedRequestKind::Independent {
            candidates: candidates.clone(),
        },
        EvaluationRequest::Pairwise {
            left, right, order, ..
        } => ResolvedRequestKind::Pairwise {
            left: *left,
            right: *right,
            order: *order,
        },
        EvaluationRequest::Listwise { candidates, .. } => ResolvedRequestKind::Listwise {
            candidates: candidates.clone(),
        },
    }
}

pub(super) fn request_granularity(
    request: &EvaluationRequest,
) -> leaven_core::AssessmentGranularity {
    match request {
        EvaluationRequest::Independent { granularity, .. }
        | EvaluationRequest::Pairwise { granularity, .. }
        | EvaluationRequest::Listwise { granularity, .. } => *granularity,
    }
}

pub(super) fn request_purpose(request: &EvaluationRequest) -> leaven_core::EvaluationPurpose {
    match request {
        EvaluationRequest::Independent { purpose, .. }
        | EvaluationRequest::Pairwise { purpose, .. }
        | EvaluationRequest::Listwise { purpose, .. } => purpose.clone(),
    }
}

pub(super) fn candidate_count(request: &ResolvedEvaluationRequest) -> usize {
    match &request.kind {
        ResolvedRequestKind::Independent { candidates }
        | ResolvedRequestKind::Listwise { candidates } => candidates.len(),
        ResolvedRequestKind::Pairwise { .. } => 2,
    }
}

pub(super) fn evaluation_cache_key<P: OptimizationProblem>(
    evaluator: leaven_kernel::Fingerprint,
    policy: CachePolicy,
    request: &ResolvedEvaluationRequest,
    graph: &RunGraphView<'_, P>,
) -> Result<EvaluationCacheKey, CacheBypassReason> {
    let candidates = request_candidate_cache_identities(&policy, request, graph)?;
    Ok(EvaluationCacheKey {
        evaluator,
        policy,
        case_set_version: request.set.case_set_version.clone(),
        case_ids: request.set.case_ids.clone(),
        candidates,
    })
}

fn request_candidate_cache_identities<P: OptimizationProblem>(
    policy: &CachePolicy,
    request: &ResolvedEvaluationRequest,
    graph: &RunGraphView<'_, P>,
) -> Result<Vec<CacheIdentity>, CacheBypassReason> {
    match policy {
        CachePolicy::Never => Err(CacheBypassReason::DisabledByPolicy),
        CachePolicy::UserKey(fingerprint) => Ok(vec![CacheIdentity::User(*fingerprint)]),
        CachePolicy::Deterministic | CachePolicy::DeterministicWithSeed(_) => {
            request_candidates(request)
                .into_iter()
                .map(|candidate| {
                    graph
                        .artifact(candidate)
                        .and_then(Artifact::cache_identity)
                        .ok_or(CacheBypassReason::MissingCandidateIdentity { candidate })
                })
                .collect()
        }
    }
}

fn request_candidates(request: &ResolvedEvaluationRequest) -> Vec<CandidateId> {
    match &request.kind {
        ResolvedRequestKind::Independent { candidates }
        | ResolvedRequestKind::Listwise { candidates } => candidates.clone(),
        ResolvedRequestKind::Pairwise { left, right, .. } => vec![*left, *right],
    }
}

pub(super) fn assessment_parts<P: OptimizationProblem>(
    assessment: Assessment<P>,
) -> (
    AssessmentRecordTarget,
    P::Evidence,
    leaven_kernel::MetadataBag,
) {
    match assessment {
        Assessment::Independent {
            candidate,
            target,
            evidence,
            metadata,
            ..
        } => (
            AssessmentRecordTarget::Independent { candidate, target },
            evidence,
            metadata,
        ),
        Assessment::Pairwise {
            left,
            right,
            target,
            evidence,
            metadata,
            ..
        } => (
            AssessmentRecordTarget::Pairwise {
                left,
                right,
                target,
            },
            evidence,
            metadata,
        ),
        Assessment::Listwise {
            candidates,
            target,
            evidence,
            metadata,
            ..
        } => (
            AssessmentRecordTarget::Listwise { candidates, target },
            evidence,
            metadata,
        ),
    }
}
