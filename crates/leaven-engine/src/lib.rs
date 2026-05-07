//! Run engine for Leaven.
//!
//! External code cannot mutate `RunGraph` directly. All mutation goes through
//! `RunContext`.

#![allow(dead_code)]

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
pub use cache::{CachePolicy, CacheStatus, EvaluationCache, EvaluationCacheKey};
pub use case_set::{CaseSet, CaseSetBuilder, EvaluationResolveError, UnsupportedEvaluationSet};
pub use context::{EvaluationContext, ProposalContext, RenderContext, RunContext, RunContextError};
pub use engine::{Engine, EngineBuilder, RunResult, optimize};
pub use events::{
    CausalInputsSummary, ErrorPolicy, EvaluationRequestSummary, RunEvent, StopReason,
};
pub use graph::storage::ApplyProposalError;
pub use graph::{
    AssessmentQuery, AssessmentView, CandidateOrigin, CandidateTree, CandidateView, FailureRef,
    Lineage, ProposalBatchView, ProposalView, RunGraph, RunGraphView,
};
pub use persistence::{RunPersistence, RunPersistenceError};
pub use reports::{
    ApplyOneReport, ApplyOutcome, ApplyReport, EvaluationReport, ProposalBatchReport,
};
pub use stage::{
    Arity, Callback, DynCallback, DynEvaluator, DynPreferenceRelation, DynProposer, DynStopper,
    EvaluationError, Evaluator, Optimizer, OptimizerError, Population, PopulationEvent,
    PopulationView, PreferenceRelation, ProposalError, Proposer, RenderError, RenderReport,
    Renderer, StepStatus, Stopper, WorkspaceRenderer,
};
pub use trust::{Actor, EvalHandle, EvidenceVisibility, ProbeRecorder, ReadScope, TrustPolicy};

pub mod prelude {
    //! Common engine imports.

    pub use crate::{
        Arity, BudgetHandle, BudgetLedger, CachePolicy, Callback, Engine, EngineBuilder,
        EvaluationContext, Evaluator, Optimizer, Population, PreferenceRelation, ProposalContext,
        Proposer, ReadScope, RenderContext, Renderer, RunContext, RunEvent, RunGraphView,
        RunResult, StepStatus, Stopper, TrustPolicy, WorkspaceRenderer, optimize,
    };
}
