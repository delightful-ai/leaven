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
pub use optimizer::{Optimizer, OptimizerError, StepStatus};
pub use population::{Population, PopulationEvent, PopulationView};
pub use preference::{DynPreferenceRelation, PreferenceRelation};
pub use proposer::{Arity, DynProposer, ProposalError, Proposer};
pub use renderer::{RenderError, RenderReport, Renderer, WorkspaceRenderer};
pub use stopper::{DynStopper, Stopper};
