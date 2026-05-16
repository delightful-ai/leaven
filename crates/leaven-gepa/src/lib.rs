//! GEPA optimizer primitives.

pub mod agent_stage;
pub mod builder;
pub mod events;
pub mod evidence;
pub mod gate;
pub mod optimizer;
pub mod part_selector;
pub mod population;
pub mod proposer;
pub mod reflection;
pub mod selector;
pub mod state;
pub mod validation;

pub use agent_stage::{GepaReflectionBootstrap, GepaStageProposer, gepa_stage_proposer};
pub use builder::{
    GepaBuilder, GepaBuilderWithPopulation, GepaBuilderWithSurface, GepaReflectWithLmBuilder,
    GepaReflectWithLmBuilderWithSurface,
};
pub use events::{GepaEventSummary, GepaSkipReason};
pub use evidence::GepaCaseEvidence;
pub use gate::{
    CheckpointGate, Gate, GateDecision, ImprovementOrEqual, NoRegression, StrictImprovement,
};
pub use optimizer::{Gepa, GepaCandidateHistoryEntry, GepaCheckpointState};
pub use part_selector::{CheckpointPartSelector, PartSelector, RoundRobinPart};
pub use population::{CheckpointPopulation, GepaPopulation};
pub use proposer::{FixedSurfaceEdit, GepaReflector, LmBackedReflector, SurfaceProposer};
pub use reflection::{
    CaseInputProjectedDataset, DEFAULT_REFLECTION_PROMPT_TEMPLATE, DefaultReflectionRenderer,
    GepaReflectiveDataset, LmBackedReflectorConfig, PlainTextEditParser, ReflectRequest,
    ReflectionError, ReflectionOutputParser, ReflectionRenderInput, ReflectionRenderer,
    ReflectiveCaseInput, ReflectiveDatasetBuilder, ReflectiveExample,
};
pub use selector::{
    CandidateSelector, CheckpointCandidateSelector, HasBestCandidate, ParetoFrequencyWeighted,
    SelectBestCandidate,
};
pub use state::{GepaCandidateIndex, GepaCandidateRecord, GepaReferenceState};
pub use validation::{FullValidation, MinibatchThenValidation, ValidationPolicy};

pub mod prelude {
    pub use crate::{
        CandidateSelector, CaseInputProjectedDataset, CheckpointCandidateSelector, CheckpointGate,
        CheckpointPopulation, DEFAULT_REFLECTION_PROMPT_TEMPLATE, DefaultReflectionRenderer,
        FixedSurfaceEdit, FullValidation, Gate, Gepa, GepaCandidateHistoryEntry,
        GepaCandidateIndex, GepaCandidateRecord, GepaCaseEvidence, GepaEventSummary,
        GepaPopulation, GepaReferenceState, GepaReflectWithLmBuilder,
        GepaReflectWithLmBuilderWithSurface, GepaReflectionBootstrap, GepaReflectiveDataset,
        GepaReflector, GepaSkipReason, GepaStageProposer, HasBestCandidate, ImprovementOrEqual,
        LmBackedReflector, LmBackedReflectorConfig, MinibatchThenValidation,
        ParetoFrequencyWeighted, PartSelector, PlainTextEditParser, ReflectRequest,
        ReflectionError, ReflectionOutputParser, ReflectionRenderInput, ReflectionRenderer,
        ReflectiveCaseInput, ReflectiveDatasetBuilder, ReflectiveExample, RoundRobinPart,
        SelectBestCandidate, StrictImprovement, SurfaceProposer, ValidationPolicy,
        gepa_stage_proposer,
    };
}
