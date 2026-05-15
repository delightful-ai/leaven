//! GEPA optimizer primitives.

pub mod agent_stage;
pub mod builder;
pub mod gate;
pub mod optimizer;
pub mod part_selector;
pub mod proposer;
pub mod reflection;
pub mod selector;
pub mod validation;

pub use agent_stage::{GepaReflectionBootstrap, GepaStageProposer, gepa_stage_proposer};
pub use builder::{
    GepaBuilder, GepaBuilderWithPopulation, GepaBuilderWithSurface, GepaReflectWithLmBuilder,
    GepaReflectWithLmBuilderWithSurface,
};
pub use gate::{
    CheckpointGate, Gate, GateDecision, ImprovementOrEqual, NoRegression, StrictImprovement,
};
pub use optimizer::{
    CheckpointPopulation, Gepa, GepaCandidateHistoryEntry, GepaCheckpointState, GepaPopulation,
    GepaScoreEvidence,
};
pub use part_selector::{CheckpointPartSelector, PartSelector, RoundRobinPart};
pub use proposer::{FixedSurfaceEdit, GepaReflector, LmBackedReflector, SurfaceProposer};
pub use reflection::{
    DEFAULT_REFLECTION_PROMPT_TEMPLATE, DefaultReflectionRenderer, GepaReflectiveDataset,
    LmBackedReflectorConfig, PlainTextEditParser, ReflectRequest, ReflectionError,
    ReflectionOutputParser, ReflectionRenderInput, ReflectionRenderer, ReflectiveDatasetBuilder,
    ReflectiveExample,
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
        FullValidation, Gate, Gepa, GepaCandidateHistoryEntry, GepaPopulation,
        GepaReflectWithLmBuilder, GepaReflectWithLmBuilderWithSurface, GepaReflectionBootstrap,
        GepaReflectiveDataset, GepaReflector, GepaStageProposer, HasBestCandidate,
        ImprovementOrEqual, LmBackedReflector, LmBackedReflectorConfig, MinibatchThenValidation,
        ParetoFrequencyWeighted, PartSelector, PlainTextEditParser, ReflectRequest,
        ReflectionError, ReflectionOutputParser, ReflectionRenderInput, ReflectionRenderer,
        ReflectiveDatasetBuilder, ReflectiveExample, RoundRobinPart, SelectBestCandidate,
        StrictImprovement, SurfaceProposer, ValidationPolicy, gepa_stage_proposer,
    };
}
