//! leaven-gepa crate skeleton.

pub struct BatchSampler;
pub struct EpochShuffled;
pub struct FixedMinibatch;
pub struct CandidateSelector;
pub struct ParetoFrequencyWeighted;
pub struct SelectBestCandidate;
pub struct Gate;
pub struct GateDecision;
pub struct ImprovementOrEqual;
pub struct NoRegression;
pub struct StrictImprovement;
pub struct Gepa;
pub struct GepaBuilder;
pub struct GepaConfig;
pub struct MergeScheduler;
pub struct SystemAwareMerge;
pub struct ReflectiveMutation;
pub struct ReflectiveMutationConfig;
pub struct PartSelector;
pub struct RoundRobinPart;
pub struct WorstEvidencePart;
pub struct FullValidation;
pub struct MinibatchThenValidation;
pub struct ValidationPolicy;
pub mod prelude {
    pub use crate::{
        BatchSampler, CandidateSelector, EpochShuffled, FullValidation, Gate, Gepa,
        ImprovementOrEqual, MinibatchThenValidation, ParetoFrequencyWeighted, PartSelector,
        ReflectiveMutation, RoundRobinPart, StrictImprovement, SystemAwareMerge, ValidationPolicy,
        WorstEvidencePart,
    };
}
