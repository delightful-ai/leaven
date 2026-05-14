//! GEPA optimizer primitives.

pub mod agent_stage;
pub mod gate;
pub mod optimizer;
pub mod part_selector;
pub mod proposer;
pub mod reflection;
pub mod selector;
pub mod validation;

pub use agent_stage::{GepaReflectionBootstrap, GepaStageProposer, gepa_stage_proposer};
pub use gate::{
    CheckpointGate, Gate, GateDecision, ImprovementOrEqual, NoRegression, StrictImprovement,
};
pub use optimizer::{
    CheckpointPopulation, Gepa, GepaBuilder, GepaCheckpointState, GepaPopulation, GepaScoreEvidence,
};
pub use part_selector::{CheckpointPartSelector, PartSelector, RoundRobinPart, WorstEvidencePart};
pub use proposer::{FixedSurfaceEdit, GepaReflector, LmBackedReflector, SurfaceProposer};
pub use reflection::{
    DEFAULT_REFLECTION_PROMPT_TEMPLATE, DefaultReflectionRenderer, GepaReflectionEvidence,
    LmBackedReflectorConfig, PlainTextEditParser, ReflectRequest, ReflectionOutputParser,
    ReflectionRenderInput, ReflectionRenderer, ReflectiveFeedbackRecord, SelectedFeedback,
};
pub use selector::{
    CandidateSelector, CheckpointCandidateSelector, HasBestCandidate, ParetoFrequencyWeighted,
    SelectBestCandidate,
};
pub use validation::{FullValidation, MinibatchThenValidation, ValidationPolicy};

pub mod prelude {
    pub use crate::{
        CandidateSelector, CheckpointCandidateSelector, CheckpointGate, CheckpointPopulation,
        DEFAULT_REFLECTION_PROMPT_TEMPLATE, DefaultReflectionRenderer, FixedSurfaceEdit,
        FullValidation, Gate, Gepa, GepaPopulation, GepaReflectionBootstrap,
        GepaReflectionEvidence, GepaReflector, GepaStageProposer, HasBestCandidate,
        ImprovementOrEqual, LmBackedReflector, LmBackedReflectorConfig, MinibatchThenValidation,
        ParetoFrequencyWeighted, PartSelector, PlainTextEditParser, ReflectRequest,
        ReflectionOutputParser, ReflectionRenderInput, ReflectionRenderer,
        ReflectiveFeedbackRecord, RoundRobinPart, SelectBestCandidate, SelectedFeedback,
        StrictImprovement, SurfaceProposer, ValidationPolicy, WorstEvidencePart,
        gepa_stage_proposer,
    };
}
