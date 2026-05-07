//! GEPA optimizer primitives.

pub mod gate;
pub mod optimizer;
pub mod part_selector;
pub mod proposer;
pub mod selector;
pub mod validation;

pub use gate::{Gate, GateDecision, ImprovementOrEqual, NoRegression, StrictImprovement};
pub use optimizer::{Gepa, GepaBuilder, GepaConfig, MergeScheduler};
pub use part_selector::{PartSelector, RoundRobinPart, WorstEvidencePart};
pub use proposer::{
    ReflectiveMutation, ReflectiveMutationConfig, SurfaceProposer, SystemAwareMerge,
};
pub use selector::{
    CandidateSelector, HasBestCandidate, ParetoFrequencyWeighted, SelectBestCandidate,
};
pub use validation::{FullValidation, MinibatchThenValidation, ValidationPolicy};

pub mod prelude {
    pub use crate::{
        CandidateSelector, FullValidation, Gate, Gepa, HasBestCandidate, ImprovementOrEqual,
        MinibatchThenValidation, ParetoFrequencyWeighted, PartSelector, ReflectiveMutation,
        RoundRobinPart, SelectBestCandidate, StrictImprovement, SurfaceProposer, SystemAwareMerge,
        ValidationPolicy, WorstEvidencePart,
    };
}
