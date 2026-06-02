//! A tiny but real GEPA-shaped accept loop driven by live seam rollouts.
//!
//! The loop is intentionally small (seed plus reflected children), but every
//! score comes from a real rollout dispatched over the bidirectional ACP seam —
//! the worker computes via `leaven/lm.complete` callbacks, not a replay map. The
//! GEPA shape is honest at this size: evaluate the parent, build per-case
//! feedback, ask the reflector for a child, screen the child on a minibatch,
//! accept only strict improvements, then re-evaluate the accepted candidate on
//! the full case set and report the best.
//!
//! This crate does not own GEPA's full search policy (parent Pareto frontier,
//! component selection, merge): that lives in `leaven-gepa`. This is the minimal
//! product-proof of the SDK bidirectional seam for the prompt/LM/exact-match
//! path.

use leaven_acp::AcpStdioProcessSession;
use serde_json::Value;

use crate::artifact::PromptArtifact;
use crate::error::StageBridgeError;
use crate::host::HostLm;
use crate::runner::{RunnerDispatch, render_prompt};

/// One case in the optimization task.
#[derive(Clone, Debug)]
pub struct OptCase {
    /// Wire case id.
    pub case_id: String,
    /// Target-free case input (rendered into the prompt by the runner).
    pub input: Value,
    /// Scorer-only target (read host-side by the reward, never sent to the worker).
    pub target: Value,
}

/// Exact-match-style reward: scores one output against the scorer-only target.
pub type RewardFn = fn(output: &str, target: &Value) -> f64;

/// Reflection: propose a child prompt from the parent and its per-case feedback.
///
/// `feedback` is the optimizer-visible `(case_input, output, score)` evidence from
/// the parent's rollouts. The reflector returns `None` when it cannot improve.
pub type ReflectFn =
    fn(parent: &PromptArtifact, feedback: &[CaseFeedback]) -> Option<PromptArtifact>;

/// Optimizer-visible feedback for one case in a candidate's evaluation.
#[derive(Clone, Debug)]
pub struct CaseFeedback {
    /// The case the feedback is for.
    pub case_id: String,
    /// The rendered prompt that produced the output.
    pub prompt: String,
    /// The candidate output the worker returned.
    pub output: String,
    /// The exact-match reward the output earned.
    pub score: f64,
}

/// A candidate in the tiny run graph.
#[derive(Clone, Debug)]
pub struct Candidate {
    /// Wire candidate id.
    pub id: String,
    /// The optimized artifact.
    pub artifact: PromptArtifact,
    /// Parent candidate id; `None` for the seed.
    pub parent_id: Option<String>,
    /// Aggregate exact-match score over the evaluated case set.
    pub score: f64,
}

/// Result of [`optimize_prompt`]: the best candidate plus the lineage frontier.
#[derive(Clone, Debug)]
pub struct Optimized {
    /// The selected best candidate.
    pub best: Candidate,
    /// Seed-to-best lineage (the candidates the loop evaluated and kept).
    pub frontier: Vec<Candidate>,
    /// Number of accept iterations the loop ran.
    pub iterations: u64,
}

/// Configuration for one tiny optimization run.
pub struct OptimizeConfig<'a, L: HostLm> {
    /// The host LM the worker calls back into.
    pub lm: &'a L,
    /// Run identity (flows into wire run/stage ids).
    pub run_id: String,
    /// The seed artifact.
    pub seed: PromptArtifact,
    /// The case set (input + scorer-only target).
    pub cases: Vec<OptCase>,
    /// Number of leading cases used to screen parent vs child.
    pub minibatch: usize,
    /// The exact-match reward.
    pub reward: RewardFn,
    /// The reflector that proposes children.
    pub reflect: ReflectFn,
    /// Maximum reflect/accept iterations.
    pub max_iterations: u64,
}

/// Runs the tiny GEPA-shaped accept loop over live seam rollouts.
///
/// Returns the best candidate after evaluating the seed and accepting any strict
/// improvements the reflector proposes.
pub fn optimize_prompt<L: HostLm>(
    session: &mut AcpStdioProcessSession,
    config: OptimizeConfig<'_, L>,
) -> Result<Optimized, StageBridgeError> {
    let OptimizeConfig {
        lm,
        run_id,
        seed,
        cases,
        minibatch,
        reward,
        reflect,
        max_iterations,
    } = config;
    if cases.is_empty() {
        return Err(StageBridgeError::optimizer("case set is empty"));
    }
    let minibatch = minibatch.clamp(1, cases.len());
    let mut dispatch = RunnerDispatch::new(lm, run_id);

    // Seed: evaluate on the full case set so the reported best score is honest.
    let seed_eval = evaluate(session, &mut dispatch, &seed, &cases, reward)?;
    let mut best = Candidate {
        id: "cand_seed".to_owned(),
        artifact: seed,
        parent_id: None,
        score: seed_eval.score,
    };
    let mut frontier = vec![best.clone()];
    let mut parent_feedback = seed_eval.feedback;
    let mut iterations = 0;

    while iterations < max_iterations {
        let Some(child_artifact) = reflect(&best.artifact, &parent_feedback) else {
            break;
        };
        iterations += 1;

        // Screen the child on the minibatch before paying for a full evaluation.
        let screen = evaluate(
            session,
            &mut dispatch,
            &child_artifact,
            &cases[..minibatch],
            reward,
        )?;
        let parent_minibatch = mean_score(&parent_feedback[..minibatch]);
        if screen.score <= parent_minibatch {
            // Not a strict improvement on the screen; stop (tiny loop, one child).
            break;
        }

        // Accepted on the screen: re-evaluate on the full case set.
        let child_eval = evaluate(session, &mut dispatch, &child_artifact, &cases, reward)?;
        let child = Candidate {
            id: format!("cand_child_{iterations}"),
            artifact: child_artifact,
            parent_id: Some(best.id.clone()),
            score: child_eval.score,
        };
        frontier.push(child.clone());
        if child.score > best.score {
            best = child;
            parent_feedback = child_eval.feedback;
        } else {
            break;
        }
    }

    Ok(Optimized {
        best,
        frontier,
        iterations,
    })
}

struct Evaluation {
    score: f64,
    feedback: Vec<CaseFeedback>,
}

/// Evaluates one artifact over a case slice via live rollouts + host scoring.
fn evaluate<L: HostLm>(
    session: &mut AcpStdioProcessSession,
    dispatch: &mut RunnerDispatch<'_, L>,
    artifact: &PromptArtifact,
    cases: &[OptCase],
    reward: RewardFn,
) -> Result<Evaluation, StageBridgeError> {
    let mut feedback = Vec::with_capacity(cases.len());
    for (index, case) in cases.iter().enumerate() {
        let candidate_id = format!("cand_eval_{index}");
        // Render the candidate artifact (host-side optimization state) against the
        // case into the model-facing prompt, then project it into the target-free
        // runner case input the worker rolls out.
        let prompt = render_prompt(artifact, &case.input);
        let projected_input = project_case_input(&case.input, &prompt)?;
        let outcome =
            dispatch.run_rollout(session, &candidate_id, &case.case_id, &projected_input)?;
        let score = reward(outcome.output(), &case.target);
        feedback.push(CaseFeedback {
            case_id: case.case_id.clone(),
            prompt,
            output: outcome.output().to_owned(),
            score,
        });
    }
    Ok(Evaluation {
        score: mean_score(&feedback),
        feedback,
    })
}

/// Projects the rendered, model-facing prompt into the runner case input.
///
/// The original case input keys are preserved; the rendered prompt is added under
/// `prompt`. The case target is never part of this projection (target-free).
fn project_case_input(case_input: &Value, prompt: &str) -> Result<Value, StageBridgeError> {
    let mut object = case_input
        .as_object()
        .cloned()
        .ok_or_else(|| StageBridgeError::optimizer("case input must be a JSON object"))?;
    object.insert("prompt".to_owned(), Value::String(prompt.to_owned()));
    Ok(Value::Object(object))
}

fn mean_score(feedback: &[CaseFeedback]) -> f64 {
    let Ok(count) = u32::try_from(feedback.len()) else {
        return 0.0;
    };
    if count == 0 {
        return 0.0;
    }
    let total: f64 = feedback.iter().map(|item| item.score).sum();
    total / f64::from(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn feedback(scores: &[f64]) -> Vec<CaseFeedback> {
        scores
            .iter()
            .enumerate()
            .map(|(index, score)| CaseFeedback {
                case_id: format!("case_{index}"),
                prompt: String::new(),
                output: String::new(),
                score: *score,
            })
            .collect()
    }

    #[test]
    fn mean_score_averages_and_handles_empty() {
        assert!((mean_score(&feedback(&[1.0, 0.0, 1.0, 0.0])) - 0.5).abs() < f64::EPSILON);
        assert!(mean_score(&feedback(&[])).abs() < f64::EPSILON);
    }

    #[test]
    fn project_case_input_adds_the_prompt_and_keeps_keys() {
        let projected = project_case_input(&json!({"question": "2 + 3"}), "P: 2 + 3").unwrap();
        assert_eq!(projected["question"], json!("2 + 3"));
        assert_eq!(projected["prompt"], json!("P: 2 + 3"));
    }

    #[test]
    fn project_case_input_rejects_non_object_input() {
        assert!(project_case_input(&json!("not an object"), "P").is_err());
    }
}
