//! `leaven-core` — cold-core types and traits for the leaven optimizer library.
//!
//! The cold core defines the algebra of optimization: artifacts, changes,
//! candidates, proposals (with causal and informational provenance),
//! assessments, evidence, populations, and the events that record run
//! truth. It does not depend on GEPA, LLM SDKs, workspace backends, or
//! any specific optimizer.
//!
//! Layout follows the v0.2.1a spec in `docs/specs/initial_library.md` and
//! the load-bearing subsystem sketch in `docs/specs/first_two_subsystems.md`.
//!
//! # Module map
//!
//! - [`ids`] — `RunId`, `CandidateId`, `ProposalId`, …
//! - [`time`] — timestamp aliases.
//! - [`metadata`] — `MetadataBag`, `BlobRef`.
//! - [`cost`] — `Cost`, budget primitives.
//! - [`error`] — `ErrorRecord`, `ErrorKind`, capability error enums.
//! - [`artifact`] — `Artifact` trait, `ContentId`.
//! - [`problem`] — `OptimizationProblem` trait.
//! - [`candidate`] — `Candidate`, `CandidateOrigin`.
//! - [`proposal`] — `Proposal`, `ProposalEffect`, `ProposalProvenance`,
//!   `CausalInputs`, `InfoRef`, `ProposalBatch`.
//! - [`evidence`] — `Evidence`, `EvidenceRef`, `EvidenceStore`.
//! - [`evaluation`] — `EvaluationRequest`, `Assessment`,
//!   `AssessmentGranularity`, `EvaluationSet`.
//! - [`preference`] — `Preference`, `PreferenceRelation` (stub).
//! - [`population`] — `PopulationEvent`, population trait surface (stub).
//! - [`graph`] — append-only run graph: storage, view, indices, events.
//! - [`context`] — `RunContext` and stage-scoped sub-contexts (stub).
//! - [`stage`] — `Proposer`, `Evaluator`, `Renderer`, `Callback`,
//!   `Stopper` trait surfaces (stubs).
//! - [`engine`] — engine shape (stub).
//! - [`result`] — final run result types (stub).
//! - [`prelude`] — common re-exports.

pub mod artifact;
pub mod candidate;
pub mod context;
pub mod cost;
pub mod engine;
pub mod error;
pub mod evaluation;
pub mod evidence;
pub mod graph;
pub mod ids;
pub mod metadata;
pub mod population;
pub mod preference;
pub mod prelude;
pub mod problem;
pub mod proposal;
pub mod result;
pub mod stage;
pub mod time;
