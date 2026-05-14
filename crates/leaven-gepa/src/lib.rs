//! GEPA optimizer primitives.

pub mod agent_stage;
pub mod gate;
pub mod optimizer;
pub mod part_selector;
pub mod proposer;
pub mod selector;
pub mod validation;

pub use agent_stage::{
    GepaReflectionBootstrap, GepaStageProposer, ReflectRequest, SelectedFeedback,
    gepa_stage_proposer,
};
pub use gate::{
    CheckpointGate, Gate, GateDecision, ImprovementOrEqual, NoRegression, StrictImprovement,
};
pub use optimizer::{
    CheckpointPopulation, Gepa, GepaBuilder, GepaCheckpointState, GepaPopulation, GepaScoreEvidence,
};
pub use part_selector::{CheckpointPartSelector, PartSelector, RoundRobinPart, WorstEvidencePart};
pub use proposer::{FixedSurfaceEdit, GepaReflector, SurfaceProposer};
pub use selector::{
    CandidateSelector, CheckpointCandidateSelector, HasBestCandidate, ParetoFrequencyWeighted,
    SelectBestCandidate,
};
pub use validation::{FullValidation, MinibatchThenValidation, ValidationPolicy};

pub mod prelude {
    pub use crate::{
        CandidateSelector, CheckpointCandidateSelector, CheckpointGate, CheckpointPopulation,
        FixedSurfaceEdit, FullValidation, Gate, Gepa, GepaPopulation, GepaReflectionBootstrap,
        GepaReflector, GepaStageProposer, HasBestCandidate, ImprovementOrEqual,
        MinibatchThenValidation, ParetoFrequencyWeighted, PartSelector, ReflectRequest,
        RoundRobinPart, SelectBestCandidate, SelectedFeedback, StrictImprovement, SurfaceProposer,
        ValidationPolicy, WorstEvidencePart, gepa_stage_proposer,
    };
}
