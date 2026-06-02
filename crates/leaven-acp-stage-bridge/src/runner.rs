//! The runner dispatch: project a rollout into `leaven/stage.run` and parse it.
//!
//! For one (candidate, case) pair this builds a target-free `RunnerRequest`,
//! dispatches it over the ACP transport, services the worker's
//! `leaven/lm.complete` callbacks against the host LM, and parses the worker's
//! text stage output into a [`RolloutOutcome`].

use std::io::{BufRead, Write};

use leaven_acp::AcpStdioSession;
use serde_json::{Value, json};

use crate::artifact::PromptArtifact;
use crate::error::StageBridgeError;
use crate::host::{HostLm, StageRunEffectHost};

/// The output one rollout produced for one case.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RolloutOutcome {
    output: String,
}

impl RolloutOutcome {
    /// The candidate's output text the rubric will score.
    pub fn output(&self) -> &str {
        &self.output
    }
}

/// Dispatches runner-stage rollouts to a worker over the ACP transport.
///
/// The dispatch is target-free: the case input rides the `RunnerRequest`, but the
/// case target never does (the runner stage guard rejects target material). The
/// rubric reads the target host-side, never the worker.
pub struct RunnerDispatch<'lm, L: HostLm> {
    host: StageRunEffectHost<'lm, L>,
    run_id: String,
    stage_seq: u64,
}

impl<'lm, L: HostLm> RunnerDispatch<'lm, L> {
    /// Binds a host LM and run identity to the dispatcher.
    pub fn new(lm: &'lm L, run_id: impl Into<String>) -> Self {
        Self {
            host: StageRunEffectHost::new(lm),
            run_id: run_id.into(),
            stage_seq: 0,
        }
    }

    /// Runs one rollout: dispatch `leaven/stage.run`, service LM callbacks, parse.
    ///
    /// `candidate_id` and `case_id` flow into the wire request; `case_input` is the
    /// projected, target-free case input the worker renders the prompt against.
    pub fn run_rollout<R: BufRead, W: Write>(
        &mut self,
        session: &mut AcpStdioSession<R, W>,
        candidate_id: &str,
        case_id: &str,
        case_input: &Value,
    ) -> Result<RolloutOutcome, StageBridgeError> {
        let stage_call_id = format!("sc_{}_{}", self.run_id, self.stage_seq);
        self.stage_seq += 1;
        let request = runner_stage_run_request(
            &self.run_id,
            &stage_call_id,
            candidate_id,
            case_id,
            case_input,
        );
        let response = session.dispatch_stage_run(&request, &self.host)?;
        let output = response.result().output();
        if output.kind() != "text" {
            return Err(StageBridgeError::output(format!(
                "runner stage output kind `{}` is not text",
                output.kind()
            )));
        }
        let text = output
            .as_value()
            .get("value")
            .and_then(Value::as_str)
            .ok_or_else(|| StageBridgeError::output("runner stage output carries no text value"))?;
        Ok(RolloutOutcome {
            output: text.to_owned(),
        })
    }
}

/// Builds the wire body of a target-free runner `leaven/stage.run` request.
fn runner_stage_run_request(
    run_id: &str,
    stage_call_id: &str,
    candidate_id: &str,
    case_id: &str,
    case_input: &Value,
) -> Value {
    json!({
        "schema_version": "leaven.stage_run.v1",
        "message": "stage_run_request",
        "stage": "runner",
        "payload": {
            "schema_version": "leaven.stage_payloads.v1",
            "role": "runner",
            "run": format!("run_{run_id}"),
            "stage_call_id": stage_call_id,
            "candidate": candidate_id,
            "case": case_id,
            "case_input": case_input,
            "target_forbidden": true
        }
    })
}

/// Renders a prompt artifact against a case input into the prompt the worker runs.
///
/// Exposed for the bridge loop and worker so prompt rendering stays one
/// definition. The case input is a flat `{key: value}` map of strings.
pub fn render_prompt(artifact: &PromptArtifact, case_input: &Value) -> String {
    let pairs: Vec<(String, String)> = case_input
        .as_object()
        .into_iter()
        .flatten()
        .filter_map(|(key, value)| value.as_str().map(|value| (key.clone(), value.to_owned())))
        .collect();
    artifact.render(&pairs)
}
