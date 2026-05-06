//! Evaluation requests and assessments.

use leaven_kernel::{
    CandidateId, CaseId, Cost, EvaluationSetId, MetadataBag, ResolvedEvaluationSetId,
};

use crate::OptimizationProblem;

#[derive(Clone, Debug)]
pub enum EvaluationSet {
    Unscoped,
    All,
    Partition(PartitionId),
    Cases(Vec<CaseId>),
    Tagged(Tag),
    Recent {
        window: Window,
    },
    Sample {
        of: Box<Self>,
        n: usize,
        seed: u64,
    },
    Stratified {
        of: Box<Self>,
        by: Tag,
        k: usize,
        seed: u64,
    },
    Union(Vec<Self>),
    Intersect(Vec<Self>),
    Difference(Box<Self>, Box<Self>),
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct PartitionId(pub smol_str::SmolStr);

impl PartitionId {
    #[must_use]
    pub fn new(name: impl Into<smol_str::SmolStr>) -> Self {
        Self(name.into())
    }
}

impl From<&'static str> for PartitionId {
    fn from(value: &'static str) -> Self {
        Self::new(value)
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Tag(pub smol_str::SmolStr);

#[derive(Clone, Debug)]
pub struct Window {
    pub limit: usize,
}

#[derive(Clone, Debug)]
pub struct ResolvedEvaluationSet {
    pub id: ResolvedEvaluationSetId,
    pub expr: EvaluationSet,
    pub case_ids: Vec<CaseId>,
    pub resolved_at: chrono::DateTime<chrono::Utc>,
    pub case_set_version: CaseSetVersion,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct CaseSetVersion(pub String);

#[derive(Clone, Debug)]
pub enum EvaluationRequest {
    Independent {
        candidates: Vec<CandidateId>,
        set: EvaluationSet,
        granularity: AssessmentGranularity,
        purpose: EvaluationPurpose,
    },
    Pairwise {
        left: CandidateId,
        right: CandidateId,
        set: EvaluationSet,
        granularity: AssessmentGranularity,
        purpose: EvaluationPurpose,
        order: PairOrder,
    },
    Listwise {
        candidates: Vec<CandidateId>,
        set: EvaluationSet,
        granularity: AssessmentGranularity,
        purpose: EvaluationPurpose,
    },
}

#[derive(Clone, Debug)]
pub struct ResolvedEvaluationRequest {
    pub kind: ResolvedRequestKind,
    pub set: ResolvedEvaluationSet,
    pub granularity: AssessmentGranularity,
    pub purpose: EvaluationPurpose,
}

#[derive(Clone, Debug)]
pub enum ResolvedRequestKind {
    Independent {
        candidates: Vec<CandidateId>,
    },
    Pairwise {
        left: CandidateId,
        right: CandidateId,
        order: PairOrder,
    },
    Listwise {
        candidates: Vec<CandidateId>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum AssessmentGranularity {
    Aggregate,
    PerCase,
    Both,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum EvaluationPurpose {
    SeedBaseline,
    Feedback,
    Screening,
    Search,
    Validation,
    FinalTest,
    Selection,
    Probe,
    Custom(smol_str::SmolStr),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum PairOrder {
    Ordered,
    Unordered,
}

#[derive(Clone, Debug)]
pub enum AssessmentTarget {
    Unscoped,
    EvaluationSet(EvaluationSetId),
    Case { set: EvaluationSetId, case: CaseId },
}

#[derive(Clone, Debug)]
pub enum Assessment<P: OptimizationProblem> {
    Independent {
        candidate: CandidateId,
        target: AssessmentTarget,
        evidence: P::Evidence,
        cost: Cost,
        metadata: MetadataBag,
    },
    Pairwise {
        left: CandidateId,
        right: CandidateId,
        target: AssessmentTarget,
        evidence: P::Evidence,
        cost: Cost,
        metadata: MetadataBag,
    },
    Listwise {
        candidates: Vec<CandidateId>,
        target: AssessmentTarget,
        evidence: P::Evidence,
        cost: Cost,
        metadata: MetadataBag,
    },
}
