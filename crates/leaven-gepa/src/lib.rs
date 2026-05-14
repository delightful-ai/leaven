//! GEPA optimizer primitives.

pub mod agent_stage;
pub mod gate;
pub mod optimizer;
pub mod part_selector;
pub mod proposer;
pub mod selector;
pub mod validation;

pub use agent_stage::{
    GepaReflectionBootstrap, GepaReflectionRequest, GepaStageProposer, gepa_stage_proposer,
};
pub use gate::{
    CheckpointGate, Gate, GateDecision, ImprovementOrEqual, NoRegression, StrictImprovement,
};
pub use optimizer::{
    CheckpointPopulation, Gepa, GepaBuilder, GepaCheckpointState, GepaConfig, GepaPopulation,
    GepaScoreEvidence, MergeScheduler,
};
pub use part_selector::{CheckpointPartSelector, PartSelector, RoundRobinPart, WorstEvidencePart};
pub use proposer::{
    ReflectiveMutation, ReflectiveMutationConfig, SurfaceProposer, SystemAwareMerge,
};
pub use selector::{
    CandidateSelector, CheckpointCandidateSelector, HasBestCandidate, ParetoFrequencyWeighted,
    SelectBestCandidate,
};
pub use validation::{FullValidation, MinibatchThenValidation, ValidationPolicy};

pub mod prelude {
    pub use crate::{
        CandidateSelector, CheckpointCandidateSelector, CheckpointGate, CheckpointPopulation,
        FullValidation, Gate, Gepa, GepaPopulation, GepaReflectionBootstrap, GepaReflectionRequest,
        GepaStageProposer, HasBestCandidate, ImprovementOrEqual, MinibatchThenValidation,
        ParetoFrequencyWeighted, PartSelector, ReflectiveMutation, RoundRobinPart,
        SelectBestCandidate, StrictImprovement, SurfaceProposer, SystemAwareMerge,
        ValidationPolicy, WorstEvidencePart, gepa_stage_proposer,
    };
}
