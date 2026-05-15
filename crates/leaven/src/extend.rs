//! Imports for users implementing a piece of the optimization machine.
//!
//! [`prelude`](crate::prelude) is for ordinary users who define a problem and
//! call [`optimize`](crate::prelude::optimize). This module is for users who
//! implement a *component* of the machine: a custom optimizer, proposer,
//! selector, gate, evaluator, materializer, store, or LM provider.
//!
//! Every name here is justified by a concrete consumer. If a symbol cannot
//! name who needs it, it does not belong in this module: make it `pub(crate)`
//! or route it through [`plumbing`](crate::plumbing) instead. The
//! `public_surface_contract` test enforces that justification.

// --- Engine stage traits: implemented by custom optimizer components. ---

/// Custom evaluator authors implement `Evaluator` to score candidates.
pub use leaven_engine::Evaluator;
/// Custom materializer authors implement `Materializer` to realize artifacts.
pub use leaven_engine::Materializer;
/// Custom optimizer authors implement `Optimizer` to drive a run loop.
pub use leaven_engine::Optimizer;
/// Population authors implement `Population` to hold and rank candidates.
pub use leaven_engine::Population;
/// Selection-policy authors implement `PreferenceRelation` to order candidates.
pub use leaven_engine::PreferenceRelation;
/// Custom proposer authors implement `Proposer` to produce candidate edits.
pub use leaven_engine::Proposer;
/// Custom renderer authors implement `Renderer` to project artifacts for stages.
pub use leaven_engine::Renderer;
/// Custom stop-condition authors implement `Stopper` to end a run loop.
pub use leaven_engine::Stopper;

// --- Engine drivers and run inspection: used by component authors and harnesses. ---

/// Proposer authors declare candidate `Arity` for their proposal batches.
pub use leaven_engine::Arity;
/// Component test harnesses construct an `Engine` to drive a run directly.
pub use leaven_engine::Engine;
/// Component test harnesses configure runs through `EngineBuilder`.
pub use leaven_engine::EngineBuilder;
/// Callback and harness authors match `RunEvent` to observe run progress.
pub use leaven_engine::RunEvent;
/// Harness code inspects `RunResult` to assert on a completed run.
pub use leaven_engine::RunResult;
/// Callback and harness authors read `StepStatus` to branch on step outcome.
pub use leaven_engine::StepStatus;

// --- Stage contexts: passed into the stage trait methods above. ---

/// `Materializer` authors return `MaterializationReport` describing the result.
pub use leaven_engine::MaterializationReport;
/// `Materializer::materialize` receives `MaterializeContext` for the workspace.
pub use leaven_engine::MaterializeContext;
/// `Materializer` authors return `MaterializeError` on failed materialization.
pub use leaven_engine::MaterializeError;
/// `Proposer::propose` receives `ProposalContext` for run state and budget.
pub use leaven_engine::ProposalContext;
/// `Renderer::render` receives `RenderContext` for the artifact projection.
pub use leaven_engine::RenderContext;
/// Stage authors receive `RunContext` as the only path to mutate `RunGraph`.
pub use leaven_engine::RunContext;
/// Stage authors read `RunGraphView` for the read-only graph projection.
pub use leaven_engine::RunGraphView;

// --- Trust, scope, and cache policy: declared by component authors. ---

/// `Evaluator` authors return `CachePolicy` to declare result cacheability.
pub use leaven_engine::CachePolicy;
/// Stage authors set `ReadScope` to bound which graph nodes a stage observes.
pub use leaven_engine::ReadScope;
/// Stage authors set `TrustPolicy` to declare how far a stage may read.
pub use leaven_engine::TrustPolicy;

// --- Cold algebra: the proposal/evaluation vocabulary a stage author emits. ---

/// Proposer authors build `CausalInputs` to record what a proposal derived from.
pub use leaven_core::CausalInputs;
/// Evaluator authors read `EvaluationRequest` to know which candidates to score.
pub use leaven_core::EvaluationRequest;
/// Evaluator authors read `EvaluationSet` to know which cases a request spans.
pub use leaven_core::EvaluationSet;
/// Proposer authors attach `InfoRef` lineage so changes carry causal inputs.
pub use leaven_core::InfoRef;
/// Evaluator and dataset authors partition cases with `PartitionId`.
pub use leaven_core::PartitionId;
/// Proposer authors build `Proposal` values describing a candidate action.
pub use leaven_core::Proposal;
/// `Proposer::propose` returns a `ProposalBatch` of sibling proposals.
pub use leaven_core::ProposalBatch;
/// Proposer authors set `ProposalBatchSemantics` to declare batch combination rules.
pub use leaven_core::ProposalBatchSemantics;
/// Proposer authors choose `ProposalEffect::Create` versus `Change` per proposal.
pub use leaven_core::ProposalEffect;
/// Proposer authors set `ProposalProvenance` to record how a proposal was made.
pub use leaven_core::ProposalProvenance;

// --- Run extension surface: store and evaluator wiring for custom workflows. ---

/// Store wiring authors implement `IntoOptimizeStore` to supply a store.
pub use leaven_run::IntoOptimizeStore;
/// Custom store authors implement `OptimizeStore` for evidence persistence.
pub use leaven_run::OptimizeStore;

// --- LM provider vocabulary: implemented and consumed by LM/agent providers. ---

/// LM provider authors implement the `Lm` trait to back optimizer LM calls.
pub use leaven_lm::Lm;
/// LM provider authors return `LmContinuation` to drive multi-turn calls.
pub use leaven_lm::LmContinuation;
/// LM provider authors return `LmError` on a failed completion call.
pub use leaven_lm::LmError;
/// LM provider authors identify a provider instance with `LmId`.
pub use leaven_lm::LmId;
/// LM provider authors accept `LmRequest` values describing a completion call.
pub use leaven_lm::LmRequest;
/// LM provider authors return `LmResponse` values from a completion call.
pub use leaven_lm::LmResponse;
/// LM provider authors build `Message` values for request and response turns.
pub use leaven_lm::Message;
/// LM provider authors build `Messages` transcripts for multi-turn requests.
pub use leaven_lm::Messages;
/// LM provider authors map `ModelName` onto a concrete provider model.
pub use leaven_lm::ModelName;
/// LM provider authors honor `OutputMode` to shape the requested response.
pub use leaven_lm::OutputMode;
/// LM provider authors read `ProviderHints` for provider-specific knobs.
pub use leaven_lm::ProviderHints;
/// LM provider authors identify the provider family with `ProviderName`.
pub use leaven_lm::ProviderName;
/// LM provider authors honor `ReasoningEffort` to set reasoning budget.
pub use leaven_lm::ReasoningEffort;
/// LM provider authors tag each `Message` with a `Role`.
pub use leaven_lm::Role;
/// LM provider authors honor `SamplingOptions` for temperature and related knobs.
pub use leaven_lm::SamplingOptions;
/// LM provider authors report `TokenUsage` so runs can meter provider cost.
pub use leaven_lm::TokenUsage;
