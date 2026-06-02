//! `leaven serve --stdio`: run the engine as the ACP client over inherited stdio.
//!
//! This is the inverse spawn direction of the bridge example: the Python SDK (or
//! any stand-in parent agent) spawns this process via `leaven serve --stdio` and
//! injects the locked capability env. This process is still the ACP *client* — it
//! runs the tiny GEPA accept loop, INITIATES `leaven/stage.run` dispatches to its
//! parent, and SERVICES the parent's `leaven/lm.complete` callbacks against a
//! deterministic host [`MockArithmeticLm`]. The parent is the ACP *agent*: it
//! serves the runner stage (the user's `@lv.runner`) and calls `leaven/lm.complete`
//! back over the seam.
//!
//! The transport is inherited stdio: stdout carries this process's JSON-RPC
//! dispatches to the parent and stdin carries the parent's responses and
//! callbacks. The optimize plan (seed + cases + loop config + named reward/reflect)
//! arrives as a `--plan` file so stdin stays a pure JSON-RPC channel; the result is
//! written to `--out` for the same reason. The LM is a deterministic mock (no
//! spend, no network); the bidirectional seam, stage dispatch, and GEPA-shaped
//! accept are real.

use std::path::{Path, PathBuf};

use leaven_acp::{AcpStdioInheritedSession, AcpTransportError};
use leaven_acp_stage_bridge::{
    CaseFeedback, MockArithmeticLm, OptCase, OptimizeConfig, Optimized, PromptArtifact, ReflectFn,
    RewardFn, StageBridgeError, optimize_prompt,
};
use leaven_public_seam::{PublicSeamError, PublicSeamPackage};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The `serve --stdio` command: drive one optimize run over the inherited seam.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServeCommand {
    /// Repo root the locked public-seam package loads from.
    pub root: PathBuf,
    /// The optimize plan (seed + cases + loop config + named reward/reflect).
    pub plan: PathBuf,
    /// Where the `Optimized` result JSON is written (stdout is the JSON-RPC channel).
    pub out: PathBuf,
}

impl ServeCommand {
    /// Runs the optimize loop as the ACP client over inherited stdio.
    ///
    /// Stdout is the JSON-RPC seam, so the operator summary is written to stderr
    /// and this returns an empty string (the full result is the `--out` file). A
    /// stray byte on stdout would corrupt the parent's JSON-RPC stream.
    pub fn run(self) -> Result<String, ServeError> {
        let plan = load_plan(&self.plan)?;
        let capability = LaunchCapability::from_env()?;
        let package = PublicSeamPackage::active_from_repo(&self.root)?;
        let profile = package.locked_acp_profile_document()?;
        let mut session = AcpStdioInheritedSession::bind(
            package,
            profile,
            capability.token,
            capability.endpoint,
            capability.fingerprint,
        )?;

        let lm = MockArithmeticLm;
        let cases = plan.optimization_cases();
        let config = OptimizeConfig {
            lm: &lm,
            run_id: plan.run_id.clone(),
            seed: PromptArtifact::new(plan.seed_template.clone()),
            cases,
            minibatch: plan.minibatch,
            reward: plan.reward.reward_fn(),
            reflect: plan.reflect.reflect_fn(),
            max_iterations: plan.max_iterations,
        };

        let optimized = optimize_prompt(session.session_mut(), config)?;
        let result = ServeResult::from_optimized(&optimized);
        let json = serde_json::to_string_pretty(&result).map_err(ServeError::SerializeResult)?;
        std::fs::write(&self.out, &json).map_err(|source| ServeError::WriteOut {
            path: self.out.clone(),
            source,
        })?;

        eprintln!(
            "leaven serve --stdio optimized over the bidirectional seam: \
             best={} score={:.3} iterations={} result={}",
            result.best.id,
            result.best.score,
            result.iterations,
            self.out.display()
        );
        // Stdout is the JSON-RPC seam; return nothing for `main` to print there.
        Ok(String::new())
    }
}

/// Locked capability env the parent injects at spawn.
struct LaunchCapability {
    token: String,
    endpoint: String,
    fingerprint: String,
}

impl LaunchCapability {
    fn from_env() -> Result<Self, ServeError> {
        Ok(Self {
            token: required_env("LEAVEN_CAPABILITY_TOKEN")?,
            endpoint: required_env("LEAVEN_ENDPOINT")?,
            fingerprint: required_env("LEAVEN_CAPABILITY_FINGERPRINT")?,
        })
    }
}

fn required_env(key: &'static str) -> Result<String, ServeError> {
    std::env::var(key).map_err(|_| ServeError::MissingCapabilityEnv { key })
}

/// The optimize plan the parent hands `leaven serve --stdio` via `--plan`.
#[derive(Clone, Debug, Deserialize)]
struct OptimizePlan {
    /// Run identity (flows into wire run/stage ids).
    run_id: String,
    /// The seed prompt template.
    seed_template: String,
    /// The case set (target-free input + scorer-only target).
    cases: Vec<PlanCase>,
    /// Number of leading cases used to screen parent vs child.
    minibatch: usize,
    /// Maximum reflect/accept iterations.
    max_iterations: u64,
    /// The named host-side reward.
    reward: RewardKind,
    /// The named host-side reflector.
    reflect: ReflectKind,
}

impl OptimizePlan {
    fn optimization_cases(&self) -> Vec<OptCase> {
        self.cases
            .iter()
            .map(|case| OptCase {
                case_id: case.case_id.clone(),
                input: case.input.clone(),
                target: case.target.clone(),
            })
            .collect()
    }
}

/// One case in the optimize plan.
#[derive(Clone, Debug, Deserialize)]
struct PlanCase {
    case_id: String,
    input: Value,
    target: Value,
}

/// The named host-side reward functions available to a serve plan.
///
/// The reward runs host-side for this slice (one scalar exact-match reward, per
/// the propagation plan). It is selected by name so the plan stays declarative and
/// the serve command owns no per-domain branching at the call site.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "snake_case")]
enum RewardKind {
    /// Exact string match against `target["answer"]`.
    ExactMatch,
}

impl RewardKind {
    fn reward_fn(self) -> RewardFn {
        match self {
            Self::ExactMatch => exact_match,
        }
    }
}

/// The named host-side reflectors available to a serve plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ReflectKind {
    /// Surface the question to the model when every parent output was empty.
    SurfaceQuestion,
}

impl ReflectKind {
    fn reflect_fn(self) -> ReflectFn {
        match self {
            Self::SurfaceQuestion => surface_question,
        }
    }
}

/// Exact-match reward: the candidate output must equal the scorer-only answer.
fn exact_match(output: &str, target: &Value) -> f64 {
    let answer = target.get("answer").and_then(Value::as_str).unwrap_or("");
    if output == answer { 1.0 } else { 0.0 }
}

/// A tiny but real reflector: when the parent never surfaced the question to the
/// model (every output is empty), propose a prompt that does. The repair is
/// derived from the feedback, not a fixed edit applied unconditionally.
fn surface_question(parent: &PromptArtifact, feedback: &[CaseFeedback]) -> Option<PromptArtifact> {
    let parent_surfaces_question = parent.template().contains("{question}");
    let every_output_empty = feedback.iter().all(|item| item.output.trim().is_empty());
    if parent_surfaces_question || !every_output_empty {
        return None;
    }
    Some(PromptArtifact::new(
        "Compute the arithmetic expression and answer with only the integer.\n\
         Expression: {question}\nAnswer:",
    ))
}

/// The `Optimized` result projected for the operator/SDK to read.
#[derive(Clone, Debug, Serialize)]
struct ServeResult {
    best: ServeCandidate,
    frontier: Vec<ServeCandidate>,
    iterations: u64,
}

impl ServeResult {
    fn from_optimized(optimized: &Optimized) -> Self {
        Self {
            best: ServeCandidate::from(&optimized.best),
            frontier: optimized
                .frontier
                .iter()
                .map(ServeCandidate::from)
                .collect(),
            iterations: optimized.iterations,
        }
    }
}

/// One candidate in the projected result.
#[derive(Clone, Debug, Serialize)]
struct ServeCandidate {
    id: String,
    parent_id: Option<String>,
    score: f64,
    template: String,
}

impl From<&leaven_acp_stage_bridge::Candidate> for ServeCandidate {
    fn from(candidate: &leaven_acp_stage_bridge::Candidate) -> Self {
        Self {
            id: candidate.id.clone(),
            parent_id: candidate.parent_id.clone(),
            score: candidate.score,
            template: candidate.artifact.template().to_owned(),
        }
    }
}

fn load_plan(path: &Path) -> Result<OptimizePlan, ServeError> {
    let bytes = std::fs::read(path).map_err(|source| ServeError::ReadPlan {
        path: path.to_path_buf(),
        source,
    })?;
    serde_json::from_slice(&bytes).map_err(|source| ServeError::ParsePlan {
        path: path.to_path_buf(),
        source,
    })
}

/// Failure raised while running `leaven serve --stdio`.
#[derive(Debug, thiserror::Error)]
pub enum ServeError {
    /// The locked capability env the parent must inject was missing.
    #[error("missing locked capability env `{key}`; the parent must inject it at spawn")]
    MissingCapabilityEnv { key: &'static str },
    /// The optimize plan file could not be read.
    #[error("failed to read optimize plan `{path}`")]
    ReadPlan {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    /// The optimize plan file was not valid JSON.
    #[error("failed to parse optimize plan `{path}`")]
    ParsePlan {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    /// The locked public-seam package or profile failed to load.
    #[error(transparent)]
    PublicSeam(#[from] PublicSeamError),
    /// The ACP transport failed (bind, dispatch, demux, or cancellation).
    #[error(transparent)]
    Transport(#[from] AcpTransportError),
    /// The optimize loop failed over the seam.
    #[error(transparent)]
    StageBridge(#[from] StageBridgeError),
    /// The result could not be serialized.
    #[error("failed to serialize the optimize result")]
    SerializeResult(#[source] serde_json::Error),
    /// The result file could not be written.
    #[error("failed to write the optimize result to `{path}`")]
    WriteOut {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn exact_match_scores_one_only_on_equality() {
        assert!((exact_match("12", &json!({"answer": "12"})) - 1.0).abs() < f64::EPSILON);
        assert!(exact_match("13", &json!({"answer": "12"})).abs() < f64::EPSILON);
        // A missing answer defaults to the empty string, so a non-empty output
        // still scores zero against it.
        assert!(exact_match("12", &json!({})).abs() < f64::EPSILON);
    }

    #[test]
    fn surface_question_repairs_only_when_every_output_is_empty() {
        let seed = PromptArtifact::new("Answer: {missing}");
        let empty = vec![CaseFeedback {
            case_id: "case_a".to_owned(),
            prompt: "Answer:".to_owned(),
            output: String::new(),
            score: 0.0,
        }];
        let repaired = surface_question(&seed, &empty).expect("empty outputs trigger a repair");
        assert!(repaired.template().contains("{question}"));

        let nonempty = vec![CaseFeedback {
            case_id: "case_a".to_owned(),
            prompt: "Answer:".to_owned(),
            output: "5".to_owned(),
            score: 1.0,
        }];
        assert!(
            surface_question(&seed, &nonempty).is_none(),
            "a producing parent needs no repair"
        );

        let already = PromptArtifact::new("Expression: {question}\nAnswer:");
        assert!(
            surface_question(&already, &empty).is_none(),
            "a parent that already surfaces the question needs no repair"
        );
    }

    #[test]
    fn plan_parses_named_reward_and_reflect() {
        let plan: OptimizePlan = serde_json::from_value(json!({
            "run_id": "serve_test",
            "seed_template": "Answer: {question}\nA:",
            "cases": [{"case_id": "case_a", "input": {"question": "2 + 3"}, "target": {"answer": "5"}}],
            "minibatch": 1,
            "max_iterations": 2,
            "reward": "exact_match",
            "reflect": "surface_question"
        }))
        .unwrap();
        assert_eq!(plan.reward, RewardKind::ExactMatch);
        assert_eq!(plan.reflect, ReflectKind::SurfaceQuestion);
        assert_eq!(plan.optimization_cases().len(), 1);
    }
}
