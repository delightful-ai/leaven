//! Bridge composing ACP stage dispatch, mock host effects, and a tiny GEPA
//! accept loop over the locked Leaven public seam.
//!
//! This crate is the host-side adapter that example 03 (`prompt optimize`) rides:
//! it projects a runner rollout into a `leaven/stage.run` dispatch, sends it to a
//! Python worker over the [`leaven_acp`] stdio transport, services the worker's
//! `leaven/lm.complete` callbacks against a deterministic host [`HostLm`], parses
//! the returned output, scores it with an exact-match reward, and runs a tiny but
//! real GEPA-shaped accept loop that produces an [`Optimized`] artifact.
//!
//! It composes transport, wire validation, and a deterministic mock LM. It does
//! not own the GEPA search policy of `leaven-gepa`, the wire contract of
//! `leaven-public-seam`, or any concrete LM/agent/sandbox provider runtime.

mod artifact;
mod error;
mod graph_host;
mod host;
mod loop_;
mod runner;

pub use artifact::PromptArtifact;
pub use error::StageBridgeError;
pub use graph_host::{RunContextGraphEffectHost, RunContextGraphEffectHostError};
pub use host::{HostLm, LmCompletionRequest, MockArithmeticLm, StageRunEffectHost};
pub use loop_::{
    Candidate, CaseFeedback, OptCase, OptimizeConfig, Optimized, ReflectFn, RewardFn,
    optimize_prompt,
};
pub use runner::{RolloutOutcome, RunnerDispatch};
