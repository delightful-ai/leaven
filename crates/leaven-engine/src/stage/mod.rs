//! Stage traits.

mod callback;
mod evaluator;
mod optimizer;
mod population;
mod preference;
mod proposer;
mod renderer;
mod stopper;

pub use callback::{Callback, DynCallback};
pub use evaluator::{DynEvaluator, EvaluationError, Evaluator};
pub use optimizer::{
    CheckpointContext, CheckpointError, CheckpointableOptimizer, Optimizer, OptimizerError,
    OptimizerStateSnapshot, PrivateStatePolicy, RestoreContext, StateFormat, StepStatus,
};
pub use population::{Population, PopulationEvent, PopulationView};
pub use preference::{DynPreferenceRelation, PreferenceRelation};
pub use proposer::{Arity, DynProposer, ProposalError, Proposer};
pub use renderer::{MaterializationReport, MaterializeError, Materializer, RenderError, Renderer};
pub use stopper::{DynStopper, Stopper};
