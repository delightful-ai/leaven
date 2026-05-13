//! GEPA optimizer primitives.

pub mod fixtures;
pub mod gate;
pub mod optimizer;
pub mod part_selector;
pub mod proposer;
pub mod selector;
pub mod validation;

pub use gate::{
    CheckpointGate, Gate, GateDecision, ImprovementOrEqual, NoRegression, StrictImprovement,
};
pub use optimizer::{
    CheckpointPopulation, Gepa, GepaBuilder, GepaCheckpointState, GepaPopulation, GepaScoreEvidence,
};
pub use part_selector::{CheckpointPartSelector, PartSelector, RoundRobinPart, WorstEvidencePart};
pub use proposer::SurfaceProposer;
pub use selector::{
    CandidateSelector, CheckpointCandidateSelector, HasBestCandidate, ParetoFrequencyWeighted,
    SelectBestCandidate,
};
pub use validation::{FullValidation, MinibatchThenValidation, ValidationPolicy};

pub mod prelude {
    //! Default GEPA imports for customizer-layer users.
    //!
    //! Fixtures (e.g. [`crate::FixedEditProposer`]) are intentionally not
    //! re-exported here; reach them through the explicit `leaven_gepa::` path.
    pub use crate::{
        CandidateSelector, CheckpointCandidateSelector, CheckpointGate, CheckpointPopulation,
        FullValidation, Gate, Gepa, GepaPopulation, HasBestCandidate, ImprovementOrEqual,
        MinibatchThenValidation, ParetoFrequencyWeighted, PartSelector, RoundRobinPart,
        SelectBestCandidate, StrictImprovement, SurfaceProposer, ValidationPolicy,
        WorstEvidencePart,
    };
}
