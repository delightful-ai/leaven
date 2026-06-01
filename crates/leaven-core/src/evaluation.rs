//! Evaluation requests and assessments.
//!
//! Evaluation has three orthogonal questions, all expressed in the
//! request:
//!
//! - **Shape** — independent scoring of N candidates, pairwise
//!   comparison of two, or listwise ranking of many. Different
//!   optimizers want different shapes; the framework refuses to
//!   silently coerce one into another.
//! - **Where** — over which cases? Expressed as an [`EvaluationSet`]
//!   that may be a static partition, a dynamic window, a sample, or
//!   a composition. Dynamic sets are *resolved* against the run's
//!   case-set version before reaching the evaluator.
//! - **Granularity** — aggregate, per-case, or both. Pareto frontiers
//!   over case-level scores need per-case evidence; budgeted scalar
//!   evaluators may only produce aggregates.
//!
//! Plus a [`purpose`](EvaluationPurpose) tag so the graph records
//! *why* an evaluation was run (validation, screening, search,
//! probing). Optimizers can filter the graph by purpose without
//! reconstructing intent from event order.

use leaven_kernel::{
    CandidateId, CaseId, Cost, EvaluationSetId, MetadataBag, ResolvedEvaluationSetId,
};

use crate::OptimizationProblem;

/// An expression describing which cases an evaluation should run over.
///
/// `EvaluationSet` is a *query*, not a list. Dynamic variants like
/// [`Recent`](EvaluationSet::Recent), [`Sample`](EvaluationSet::Sample),
/// and [`Stratified`](EvaluationSet::Stratified) describe a
/// resolution strategy that the run context evaluates against the
/// current case-set version, freezing the result into a
/// [`ResolvedEvaluationSet`] that the evaluator actually sees.
///
/// # Composition
///
/// `Union`, `Intersect`, and `Difference` compose set expressions.
/// Composition is recursive — a `Sample` of a `Union` of two
/// `Tagged`s is well-formed and resolves bottom-up.
///
/// # Caching
///
/// Cache keys use [`ResolvedEvaluationSetId`], not the unresolved
/// expression. Two identical `Recent { window }` requests issued at
/// different iterations resolve to different IDs and do not pool.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum EvaluationSet {
    /// No dataset scope (single-task or evaluator-internal).
    Unscoped,
    /// Every case in the case set.
    All,
    /// All cases in the named partition (e.g. `TRAIN`, `VALIDATION`).
    Partition(PartitionId),
    /// An explicit case-id list.
    Cases(Vec<CaseId>),
    /// All cases bearing the named tag.
    Tagged(Tag),
    /// Most-recent cases under some run-defined window.
    Recent {
        /// Recency bound.
        window: Window,
    },
    /// Random subsample of `n` cases from `of`, drawn with `seed`.
    Sample {
        /// Set to sample from.
        of: Box<Self>,
        /// Number of cases to draw.
        n: usize,
        /// Deterministic seed.
        seed: u64,
    },
    /// Stratified subsample drawing `k` cases per `by`-tagged stratum.
    Stratified {
        /// Set to sample from.
        of: Box<Self>,
        /// Tag whose values define strata.
        by: Tag,
        /// Cases per stratum.
        k: usize,
        /// Deterministic seed.
        seed: u64,
    },
    /// Union of multiple sets (deduplicated by case id).
    Union(Vec<Self>),
    /// Intersection of multiple sets.
    Intersect(Vec<Self>),
    /// Set difference: cases in the first but not the second.
    Difference(Box<Self>, Box<Self>),
}

/// Named partition of a case set.
///
/// Reserved names like `TRAIN`, `VALIDATION`, and `TEST` are
/// conventional rather than enforced — partitions are user-defined
/// strings so a run can carve its case set however it needs (e.g.
/// `SEARCH`, `HOLDOUT`, `PROBE`). Trust policies and frontier filters
/// often key on partition identity.
#[derive(
    Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, serde::Serialize, serde::Deserialize,
)]
pub struct PartitionId(pub smol_str::SmolStr);

impl PartitionId {
    /// Constructs a partition id from any string-like value.
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

/// Free-form tag attached to one or more cases.
///
/// Tags label cases for ad-hoc filtering and stratification — language,
/// difficulty class, source corpus, anything user-meaningful. Many
/// cases may share a tag and one case may carry multiple tags.
#[derive(Clone, Debug, Eq, PartialEq, Hash, serde::Serialize, serde::Deserialize)]
pub struct Tag(pub smol_str::SmolStr);

/// Recency window for [`EvaluationSet::Recent`].
///
/// Currently a count-based bound. Future variants (time-based, version-
/// based) can extend the type without breaking call sites that only
/// care about the count.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Window {
    /// Maximum number of recent cases to include.
    pub limit: usize,
}

/// A frozen snapshot of an evaluation-set query.
///
/// Produced by the run context when a dynamic [`EvaluationSet`] is
/// resolved against the current case-set version. Evaluators always
/// see resolved sets, never expressions — caching, comparison, and
/// reproduction all key on the resolved id.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ResolvedEvaluationSet {
    /// Stable id for this resolution.
    pub id: ResolvedEvaluationSetId,
    /// The original (possibly dynamic) expression.
    pub expr: EvaluationSet,
    /// Concrete case ids the expression resolved to.
    pub case_ids: Vec<CaseId>,
    /// Resolution wall-clock time, recorded for telemetry.
    pub resolved_at: chrono::DateTime<chrono::Utc>,
    /// Version of the case set that produced this resolution.
    pub case_set_version: CaseSetVersion,
}

/// Opaque version tag for a case set.
///
/// Bumped whenever the underlying case set changes (cases added,
/// removed, retagged). Cache keys include the version so resolutions
/// against a stale case set don't accidentally hit cache entries from
/// a newer one.
#[derive(
    Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, serde::Serialize, serde::Deserialize,
)]
pub struct CaseSetVersion(pub String);

/// What an optimizer wants from an evaluator.
///
/// The shape of the request matches the shape of the evidence the
/// optimizer needs:
///
/// - `Independent` — score each candidate on its own. N candidates
///   produce N independent assessments.
/// - `Pairwise` — compare two candidates head-to-head. One assessment
///   that talks about both.
/// - `Listwise` — rank a list of candidates together. One assessment
///   that ranks the whole group.
///
/// Independent over `[A, B]` is *not* the same as Pairwise over
/// `(A, B)`. Choosing the wrong shape produces evidence the optimizer
/// can't use; the framework will not silently coerce one into another.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum EvaluationRequest {
    /// Score each candidate independently.
    Independent {
        /// Candidates to score.
        candidates: Vec<CandidateId>,
        /// Cases to evaluate on.
        set: EvaluationSet,
        /// Aggregate vs per-case evidence.
        granularity: AssessmentGranularity,
        /// Why this evaluation is being run.
        purpose: EvaluationPurpose,
    },
    /// Compare two candidates head-to-head.
    Pairwise {
        /// Left-hand candidate.
        left: CandidateId,
        /// Right-hand candidate.
        right: CandidateId,
        /// Cases to evaluate on.
        set: EvaluationSet,
        /// Aggregate vs per-case evidence.
        granularity: AssessmentGranularity,
        /// Why this evaluation is being run.
        purpose: EvaluationPurpose,
        /// Whether order is meaningful.
        order: PairOrder,
    },
    /// Rank a list of candidates together.
    Listwise {
        /// Candidates to rank.
        candidates: Vec<CandidateId>,
        /// Cases to evaluate on.
        set: EvaluationSet,
        /// Aggregate vs per-case evidence.
        granularity: AssessmentGranularity,
        /// Why this evaluation is being run.
        purpose: EvaluationPurpose,
    },
}

/// An evaluation request after the run context has resolved its
/// [`EvaluationSet`] against the current case-set version.
///
/// What evaluators actually receive: the resolved set, plus the same
/// granularity and purpose the requester specified. Evaluators do not
/// see the original (possibly dynamic) expression; the run graph
/// records both for retrospective analysis.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ResolvedEvaluationRequest {
    /// Shape of the request after resolution.
    pub kind: ResolvedRequestKind,
    /// Concrete cases the evaluator runs on.
    pub set: ResolvedEvaluationSet,
    /// Aggregate vs per-case evidence.
    pub granularity: AssessmentGranularity,
    /// Why this evaluation is being run.
    pub purpose: EvaluationPurpose,
}

/// Resolved counterpart of [`EvaluationRequest`]. The variants mirror
/// the unresolved shapes, minus the [`EvaluationSet`] (which has been
/// resolved into [`ResolvedEvaluationRequest::set`]).
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum ResolvedRequestKind {
    /// Independent scoring of each candidate.
    Independent {
        /// Candidates to score.
        candidates: Vec<CandidateId>,
    },
    /// Pairwise comparison of two candidates.
    Pairwise {
        /// Left-hand candidate.
        left: CandidateId,
        /// Right-hand candidate.
        right: CandidateId,
        /// Whether order is meaningful.
        order: PairOrder,
    },
    /// Listwise ranking of a candidate group.
    Listwise {
        /// Candidates to rank.
        candidates: Vec<CandidateId>,
    },
}

/// Whether the optimizer wants per-set, per-case, or both shapes of
/// evidence.
///
/// Pareto frontiers over case-level scores need `PerCase`. A scalar
/// keep-best optimizer can do its job with `Aggregate`. `Both`
/// requests both shapes when the evaluator can supply them. An
/// evaluator that cannot satisfy the requested granularity should
/// surface that as an explicit error rather than silently substituting
/// a different shape.
#[derive(
    Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, serde::Serialize, serde::Deserialize,
)]
pub enum AssessmentGranularity {
    /// One assessment per candidate over the whole resolved set.
    Aggregate,
    /// One assessment per (candidate, case) pair.
    PerCase,
    /// Both aggregate and per-case assessments.
    Both,
}

/// Why an evaluation is being run.
///
/// Recorded on every assessment so the graph can answer questions
/// like "what was the test-set score of the eventual winner" without
/// reconstructing intent from event order. Trust policies and
/// frontier filters often key on purpose (e.g. "frontier ignores
/// `Probe`-purpose assessments").
#[derive(
    Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, serde::Serialize, serde::Deserialize,
)]
pub enum EvaluationPurpose {
    /// Initial scoring of seed candidates.
    SeedBaseline,
    /// Producing evidence the proposer will read.
    Feedback,
    /// Cheap pre-validation screen.
    Screening,
    /// Search-time evaluation that informs population updates.
    Search,
    /// Held-out validation evaluation.
    Validation,
    /// One-shot test-set evaluation, typically post-run.
    FinalTest,
    /// Evaluation used to pick a winner among candidates.
    Selection,
    /// Exploratory evaluation outside the standard search loop.
    Probe,
    /// User-defined purpose.
    Custom(smol_str::SmolStr),
}

/// Whether order is meaningful in a pairwise comparison.
///
/// `Ordered` means `(left, right)` and `(right, left)` are different
/// requests; the evaluator may produce different evidence depending
/// on order (LLM judges often have positional bias). `Unordered`
/// means the evaluator declares its judgment is symmetric, which lets
/// the cache pool both orderings.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, serde::Serialize, serde::Deserialize)]
pub enum PairOrder {
    /// Order matters; the evaluator may be asymmetric.
    Ordered,
    /// Order is not meaningful; the evaluator is symmetric.
    Unordered,
}

/// What an [`Assessment`] is *about*.
///
/// `Unscoped` is the single-task case (no dataset). `EvaluationSet`
/// targets the whole resolved set. `Case` targets one case within a
/// set — used for per-case evidence in `PerCase` granularity.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum AssessmentTarget {
    /// No dataset scope.
    Unscoped,
    /// The whole evaluation set.
    EvaluationSet(EvaluationSetId),
    /// A specific case within a set.
    Case {
        /// Set the case belongs to.
        set: EvaluationSetId,
        /// Case identifier.
        case: CaseId,
    },
}

/// One unit of evaluation output recorded in the run graph.
///
/// Variants mirror [`EvaluationRequest`]: an `Independent` request
/// yields `Independent` assessments, `Pairwise` yields `Pairwise`,
/// `Listwise` yields `Listwise`. Every assessment carries its
/// [`P::Evidence`], the [`Cost`] charged to produce it, and a
/// [`MetadataBag`] for breadcrumbs.
///
/// [`P::Evidence`]: OptimizationProblem::Evidence
#[derive(Clone, Debug)]
pub enum Assessment<P: OptimizationProblem> {
    /// Independent scoring of one candidate.
    Independent {
        /// Candidate scored.
        candidate: CandidateId,
        /// What the assessment is about.
        target: AssessmentTarget,
        /// Evidence the evaluator produced.
        evidence: P::Evidence,
        /// Cost charged to produce the evidence.
        cost: Cost,
        /// Operational metadata.
        metadata: MetadataBag,
    },
    /// Pairwise comparison between two candidates.
    Pairwise {
        /// Left-hand candidate.
        left: CandidateId,
        /// Right-hand candidate.
        right: CandidateId,
        /// What the assessment is about.
        target: AssessmentTarget,
        /// Evidence describing the comparison.
        evidence: P::Evidence,
        /// Cost charged.
        cost: Cost,
        /// Operational metadata.
        metadata: MetadataBag,
    },
    /// Listwise ranking of a candidate group.
    Listwise {
        /// Candidates ranked, in input order.
        candidates: Vec<CandidateId>,
        /// What the assessment is about.
        target: AssessmentTarget,
        /// Evidence describing the ranking.
        evidence: P::Evidence,
        /// Cost charged.
        cost: Cost,
        /// Operational metadata.
        metadata: MetadataBag,
    },
}
