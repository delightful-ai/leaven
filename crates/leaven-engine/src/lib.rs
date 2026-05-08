//! Run engine for Leaven.
//!
//! External code cannot mutate `RunGraph` directly. All mutation goes through
//! `RunContext`.

mod budget;
mod cache;
mod case_set;
mod context;
mod engine;
mod events;
mod graph;
mod persistence;
mod reports;
mod stage;
mod trust;

pub use budget::{BudgetHandle, BudgetLedger};
pub use cache::{
    CacheBypassReason, CachePolicy, CacheStatus, EvaluationCache, EvaluationCacheEntry,
    EvaluationCacheKey, EvaluationCacheSnapshot,
};
pub use case_set::{CaseSet, CaseSetBuilder, EvaluationResolveError, UnsupportedEvaluationSet};
pub use context::{
    EvaluationContext, MaterializeContext, ProposalContext, RenderContext, RunContext,
    RunContextError,
};
pub use engine::{Engine, EngineBuilder, RunResult, optimize};
pub use events::{
    CausalInputsSummary, ErrorPolicy, EvaluationRequestSummary, RunEvent, StopReason,
};
pub use graph::storage::ApplyProposalError;
pub use graph::{
    AssessmentQuery, AssessmentView, CandidateOrigin, CandidateTree, CandidateView,
    EvaluationRequestView, FailureRef, Lineage, ProposalBatchView, ProposalView, RunGraph,
    RunGraphView,
};
pub use persistence::{
    CacheIndexSnapshot, GraphSnapshotRef, RunCheckpoint, RunCheckpointRequest, RunPersistence,
    RunPersistenceError, StageJournalSnapshot, StageStateSnapshot, WorkspaceJournalSnapshot,
};
pub use reports::{
    ApplyOneReport, ApplyOutcome, ApplyReport, EvaluationReport, ProposalBatchReport,
};
pub use stage::{
    Arity, Callback, DynCallback, DynEvaluator, DynPreferenceRelation, DynProposer, DynStopper,
    EvaluationError, Evaluator, MaterializationReport, MaterializeError, Materializer, Optimizer,
    OptimizerError, Population, PopulationEvent, PopulationView, PreferenceRelation, ProposalError,
    Proposer, RenderError, Renderer, StepStatus, Stopper,
};
pub use stage::{
    CheckpointContext, CheckpointError, CheckpointableOptimizer, OptimizerStateSnapshot,
    PrivateStatePolicy, RestoreContext, StateFormat,
};
pub use trust::{
    Actor, EvalHandle, EvidenceVisibility, ProbeRecorder, ReadScope, TrustPolicy, TrustViolation,
};

pub mod prelude {
    //! Common engine imports.

    pub use crate::{
        Arity, BudgetHandle, BudgetLedger, CacheBypassReason, CachePolicy, CacheStatus, Callback,
        CheckpointContext, CheckpointError, CheckpointableOptimizer, Engine, EngineBuilder,
        EvaluationCacheSnapshot, EvaluationContext, Evaluator, GraphSnapshotRef,
        MaterializationReport, MaterializeContext, MaterializeError, Materializer, Optimizer,
        OptimizerStateSnapshot, Population, PreferenceRelation, PrivateStatePolicy,
        ProposalContext, Proposer, ReadScope, RenderContext, Renderer, RestoreContext,
        RunCheckpoint, RunContext, RunEvent, RunGraphView, RunResult, StateFormat, StepStatus,
        Stopper, TrustPolicy, optimize,
    };
}
