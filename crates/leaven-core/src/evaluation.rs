//! Evaluation request/assessment shapes.
//!
//! The request shape says **what kind of evaluation is intended**:
//! independent scoring, pairwise comparison, or listwise ranking. The
//! granularity says whether the result is per-case (GEPA-style) or
//! aggregated. The set selects which cases participate.
//!
//! Resolution is explicit. `EvaluationRequest` may name a dynamic set
//! ("all cases not yet evaluated for this candidate"); the
//! [`crate::context::RunContext`] resolves that to a frozen
//! [`ResolvedEvaluationRequest`] *before* calling the evaluator. Cache
//! keys are computed against the resolved form.

use serde::{Deserialize, Serialize};

use crate::cost::Cost;
use crate::evidence::{Evidence, EvidenceRef};
use crate::ids::{CandidateId, PartitionId};
use crate::metadata::MetadataBag;

/// Whether the evaluation result is per-case, aggregated, or both.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum AssessmentGranularity {
    /// Aggregate across the evaluation set; one assessment per
    /// candidate (or per pair / list).
    Aggregate,
    /// One assessment per case (GEPA-style instance-wise frontiers).
    PerCase,
    /// Both shapes returned. Required by some optimizers that need
    /// per-case for selection and aggregate for reporting.
    Both,
}

/// Why the evaluation was requested. Influences trust checks (e.g.
/// `Holdout` may be visible only to the engine, not to proposers).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum EvaluationPurpose {
    /// Used by the optimizer's selection / search loop.
    Search,
    /// Held-out evaluation; results often partition-protected.
    Holdout,
    /// User-driven probe (debugging, analysis).
    Probe,
}

/// Order of a pairwise comparison.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum PairOrder {
    /// `(left, right)` order matters: e.g. left is incumbent, right
    /// is challenger.
    Ordered,
    /// `(left, right)` and `(right, left)` are interchangeable.
    Symmetric,
}

/// Where to evaluate. May be expressed in terms the engine resolves at
/// request time (a partition name, a sampled subset, …).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum EvaluationSet {
    /// All cases.
    All,
    /// All cases in a named partition.
    Partition(PartitionId),
    /// Explicit subset by index.
    Indices(Vec<usize>),
    /// Sample `n` random cases from the named partition with the given
    /// seed.
    Sample {
        partition: PartitionId,
        n: usize,
        seed: u64,
    },
}

/// Resolved (frozen) form of an evaluation set. Constructed by
/// `RunContext` before the evaluator runs. Cache keys hash the
/// resolved form.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResolvedEvaluationSet {
    /// Stable identity for cache keys. Constructed deterministically
    /// from the original expression plus the case-set version.
    pub id: ResolvedEvaluationSetId,
    /// Indices into the run's case set.
    pub indices: Vec<usize>,
    /// Original expression, kept for graph durability.
    pub original: EvaluationSet,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ResolvedEvaluationSetId(pub String);

#[derive(Clone, Debug)]
pub enum EvaluationRequest {
    /// Independent scoring of one or more candidates. Each candidate
    /// produces its own assessment(s).
    Independent {
        candidates: Vec<CandidateId>,
        set: EvaluationSet,
        granularity: AssessmentGranularity,
        purpose: EvaluationPurpose,
    },

    /// One pairwise comparison: a single assessment over two candidates.
    Pairwise {
        left: CandidateId,
        right: CandidateId,
        set: EvaluationSet,
        granularity: AssessmentGranularity,
        purpose: EvaluationPurpose,
        order: PairOrder,
    },

    /// One listwise ranking over `n` candidates.
    Listwise {
        candidates: Vec<CandidateId>,
        set: EvaluationSet,
        granularity: AssessmentGranularity,
        purpose: EvaluationPurpose,
    },
}

#[derive(Clone, Debug)]
pub struct ResolvedEvaluationRequest {
    pub original: EvaluationRequest,
    pub resolved_set: ResolvedEvaluationSet,
}

/// What an assessment is *about* — whole-set aggregate, per-case score,
/// pair-of-pair output, etc.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum AssessmentTarget {
    /// Aggregate over the evaluation set.
    Aggregate {
        set: ResolvedEvaluationSetId,
    },
    /// One specific case.
    Case {
        set: ResolvedEvaluationSetId,
        case_index: usize,
    },
}

/// Live evaluator output before storage normalization. Carries inline
/// evidence; the context converts it to a [`StoredAssessment`] before
/// graph insertion.
#[derive(Clone, Debug)]
pub enum Assessment<E: Evidence> {
    Independent {
        candidate: CandidateId,
        target: AssessmentTarget,
        evidence: E,
        cost: Cost,
        metadata: MetadataBag,
    },
    Pairwise {
        left: CandidateId,
        right: CandidateId,
        target: AssessmentTarget,
        evidence: E,
        cost: Cost,
        metadata: MetadataBag,
    },
    Listwise {
        candidates: Vec<CandidateId>,
        target: AssessmentTarget,
        evidence: E,
        cost: Cost,
        metadata: MetadataBag,
    },
}

/// Graph-durable form: evidence is replaced with an `EvidenceRef`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum StoredAssessment {
    Independent {
        candidate: CandidateId,
        target: AssessmentTarget,
        evidence: EvidenceRef,
        cost: Cost,
        metadata: MetadataBag,
    },
    Pairwise {
        left: CandidateId,
        right: CandidateId,
        target: AssessmentTarget,
        evidence: EvidenceRef,
        cost: Cost,
        metadata: MetadataBag,
    },
    Listwise {
        candidates: Vec<CandidateId>,
        target: AssessmentTarget,
        evidence: EvidenceRef,
        cost: Cost,
        metadata: MetadataBag,
    },
}
