//! Behavior-bearing GEPA optimizer primitives.
//!
//! This crate owns GEPA loop state, surface-edit lowering, selection,
//! validation, reflection request construction, report projection, and
//! checkpoint state. Some product-facing slots remain scaffold or advanced
//! extension points; see the crate-local `AGENTS.md` before widening exports.

pub mod agent_stage;
pub mod builder;
pub mod events;
pub mod evidence;
pub mod gate;
pub mod optimizer;
pub mod part_selector;
pub mod population;
mod proposer;
mod python_random;
pub mod reflection;
pub mod report;
pub mod selector;
pub mod state;
pub mod validation;

pub use builder::{
    GepaBuilder, GepaBuilderWithPopulation, GepaBuilderWithSurface, GepaReferenceBuilder,
    GepaReferenceBuilderWithSurface, GepaReflectWithLmBuilder, GepaReflectWithLmBuilderWithSurface,
};
pub use events::{GepaEventSummary, GepaSkipReason};
pub use evidence::GepaCaseEvidence;
pub use gate::{
    CheckpointGate, Gate, GateDecision, ImprovementOrEqual, NoRegression, StrictImprovement,
};
pub use optimizer::{
    Gepa, GepaCandidateHistoryEntry, GepaCheckpointState, GepaProfile, GepaProposalAttempt,
};
pub use part_selector::{CheckpointPartSelector, PartSelector, RoundRobinPart};
pub use population::{CheckpointPopulation, GepaPopulation};
#[doc(hidden)]
pub use proposer::MissingReflector;
pub use proposer::{GepaReflector, LmBackedReflector, SurfaceProposer};
pub use reflection::{
    Attachment, AttachmentKind, CaseInputProjectedDataset, Check, Checks,
    DEFAULT_REFLECTION_PROMPT_TEMPLATE, DefaultReflectionRenderer, GepaReflectiveDataset,
    LmBackedReflectorConfig, PlainTextEditParser, ReflectRequest, ReflectionError,
    ReflectionOutputParser, ReflectionRenderInput, ReflectionRenderer, ReflectiveCase,
    ReflectiveCaseInput, ReflectiveDatasetBuilder, ReflectiveRun, ReflectiveSideInfoValue,
    ReflectiveValue,
};
pub use report::{
    GepaReport, GepaReportCandidate, GepaReportFrontierCase, GepaReportHistoryEntry,
    GepaReportProfile, GepaReportQualitySummary, GepaReportValidationSubscore,
};
pub use selector::{
    CandidateSelector, CheckpointCandidateSelector, HasBestCandidate, PopulationBestFallback,
    SelectBestCandidate,
};
pub use state::{GepaCandidateIndex, GepaCandidateRecord, GepaReferenceState};
pub use validation::{FullValidation, MinibatchThenValidation, ValidationPolicy};

/// Explicit scaffold and test-support fixtures.
pub mod test_support {
    pub use crate::proposer::FixedSurfaceEdit;
}

pub mod prelude {
    pub use crate::{
        CaseInputProjectedDataset, CheckpointGate, CheckpointPopulation,
        DEFAULT_REFLECTION_PROMPT_TEMPLATE, DefaultReflectionRenderer, FullValidation, Gate, Gepa,
        GepaCandidateHistoryEntry, GepaCandidateIndex, GepaCandidateRecord, GepaCaseEvidence,
        GepaEventSummary, GepaPopulation, GepaProfile, GepaReferenceBuilder,
        GepaReferenceBuilderWithSurface, GepaReferenceState, GepaReflectWithLmBuilder,
        GepaReflectWithLmBuilderWithSurface, GepaReflectiveDataset, GepaReflector, GepaReport,
        GepaReportCandidate, GepaReportFrontierCase, GepaReportHistoryEntry, GepaReportProfile,
        GepaReportQualitySummary, GepaReportValidationSubscore, GepaSkipReason, ImprovementOrEqual,
        LmBackedReflector, LmBackedReflectorConfig, PartSelector, PlainTextEditParser,
        ReflectRequest, ReflectionError, ReflectionOutputParser, ReflectionRenderInput,
        ReflectionRenderer, ReflectiveCase, ReflectiveCaseInput, ReflectiveDatasetBuilder,
        ReflectiveRun, ReflectiveSideInfoValue, ReflectiveValue, RoundRobinPart, StrictImprovement,
        SurfaceProposer, ValidationPolicy,
    };
}
