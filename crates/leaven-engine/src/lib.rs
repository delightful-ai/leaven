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
mod sqlite_cache;
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
    RunContextError, StageEngineContext,
};
pub use engine::{Engine, EngineBuilder, RunResult, optimize};
pub use events::{
    CausalInputsSummary, ErrorPolicy, EvaluationRequestSummary, RunEvent, StopReason,
};
pub use graph::storage::ApplyProposalError;
pub use graph::{
    AssessmentQuery, AssessmentView, CandidateOrigin, CandidateTree, CandidateView,
    EvaluationRequestView, FailureRef, Lineage, ProposalBatchView, ProposalView, RunGraph,
    RunGraphRestoreError, RunGraphSnapshot, RunGraphView, ScopedRunGraphView,
};
pub use persistence::{
    CacheIndexSnapshot, GraphSnapshotRef, OptimizerStateWrite, RestoredRunState, RunCheckpoint,
    RunCheckpointRequest, RunPersistence, RunPersistenceError, StageJournalSnapshot,
    StageStateSnapshot, StoreRunPersistence, WorkspaceJournalSnapshot,
};
pub use reports::{
    ApplyOneReport, ApplyOutcome, ApplyReport, CasewiseEvaluationReport, EvaluationReport,
    ProposalBatchReport,
};
pub use sqlite_cache::{EvaluationCacheStoreError, SqliteEvaluationCache};
pub use stage::{
    Arity, Callback, DynCallback, DynEvaluator, DynPreferenceRelation, DynProposer, DynStopper,
    EvaluationError, Evaluator, MaterializationReport, MaterializeError, Materializer, Optimizer,
    OptimizerError, OptimizerReport, OptimizerReportPayload, OptimizerStateReader, Population,
    PopulationEvent, PopulationView, PreferenceRelation, ProposalError, Proposer, RenderError,
    Renderer, StepStatus, Stopper,
};
pub use stage::{
    CheckpointContext, CheckpointError, CheckpointableOptimizer, OptimizerStateSnapshot,
    PrivateStatePolicy, RestoreContext, StateFormat, restore_checkpointable_optimizer_state,
};
pub use trust::{
    Actor, EvalHandle, EvidenceVisibility, ProbeRecorder, ReadScope, TrustPolicy, TrustViolation,
};

pub mod prelude {
    //! Common engine imports.

    pub use crate::{
        Arity, BudgetHandle, BudgetLedger, CacheBypassReason, CachePolicy, CacheStatus, Callback,
        CheckpointContext, CheckpointError, CheckpointableOptimizer, Engine, EngineBuilder,
        EvaluationCacheSnapshot, EvaluationCacheStoreError, EvaluationContext, Evaluator,
        GraphSnapshotRef, MaterializationReport, MaterializeContext, MaterializeError,
        Materializer, Optimizer, OptimizerReport, OptimizerReportPayload, OptimizerStateReader,
        OptimizerStateSnapshot, Population, PreferenceRelation, PrivateStatePolicy,
        ProposalContext, Proposer, ReadScope, RenderContext, Renderer, RestoreContext,
        RestoredRunState, RunCheckpoint, RunContext, RunEvent, RunGraphRestoreError,
        RunGraphSnapshot, RunGraphView, RunResult, ScopedRunGraphView, SqliteEvaluationCache,
        StageEngineContext, StateFormat, StepStatus, Stopper, StoreRunPersistence, TrustPolicy,
        optimize, restore_checkpointable_optimizer_state,
    };
}
