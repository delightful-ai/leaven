use std::io::Write;
use std::process::{Command, Stdio};

use leaven_public_seam::PublicSeamError;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// Configured runner-stage execution for public-seam service processes.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SeamStageConfig {
    /// No stage runner is wired.
    None,
    /// Deterministic runner output. This is mechanics evidence, not Python worker proof.
    MockRunner {
        /// Text returned as the runner output value.
        text: String,
        /// Short output summary.
        summary: String,
    },
    /// Execute runner dispatches through an external JSON-RPC worker process.
    CommandRunner {
        /// Command argv. The first item is the program; remaining items are arguments.
        argv: Vec<String>,
    },
}

impl SeamStageConfig {
    pub(crate) fn runner_result(&self, params: &Value) -> Result<Value, PublicSeamError> {
        match self {
            Self::None => Err(PublicSeamError::InvalidPlan {
                message: "configured seam service does not provide a stage runner".to_owned(),
            }),
            Self::MockRunner { text, summary } => {
                Ok(stage_run_text_result(stage_call_id(params)?, text, summary))
            }
            Self::CommandRunner { argv } => command_runner_result(argv, params),
        }
    }
}

impl Default for SeamStageConfig {
    fn default() -> Self {
        Self::None
    }
}

fn command_runner_result(argv: &[String], params: &Value) -> Result<Value, PublicSeamError> {
    let (program, args) = argv
        .split_first()
        .ok_or_else(|| PublicSeamError::InvalidPlan {
            message: "configured command runner argv must not be empty".to_owned(),
        })?;
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| PublicSeamError::InvalidPlan {
            message: format!("failed to spawn configured stage runner `{program}`: {error}"),
        })?;
    let request = stage_run_json_rpc_request(params);
    let request_line =
        serde_json::to_vec(&request).map_err(|error| PublicSeamError::InvalidStageRun {
            message: format!("stage.run request serialization failed: {error}"),
        })?;
    {
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| PublicSeamError::InvalidPlan {
                message: "configured stage runner stdin was not piped".to_owned(),
            })?;
        stdin
            .write_all(&request_line)
            .and_then(|()| stdin.write_all(b"\n"))
            .map_err(|error| PublicSeamError::InvalidPlan {
                message: format!("failed to write stage.run request to `{program}`: {error}"),
            })?;
    }
    let output = child
        .wait_with_output()
        .map_err(|error| PublicSeamError::InvalidPlan {
            message: format!("failed waiting for configured stage runner `{program}`: {error}"),
        })?;
    if !output.status.success() {
        return Err(PublicSeamError::InvalidPlan {
            message: format!(
                "configured stage runner `{program}` exited with {}: {}",
                output.status,
                String::from_utf8_lossy(&output.stderr).trim()
            ),
        });
    }
    let stdout =
        String::from_utf8(output.stdout).map_err(|error| PublicSeamError::InvalidPlan {
            message: format!(
                "configured stage runner `{program}` emitted non-UTF-8 stdout: {error}"
            ),
        })?;
    let response_line = stdout
        .lines()
        .find(|line| !line.trim().is_empty())
        .ok_or_else(|| PublicSeamError::InvalidPlan {
            message: format!("configured stage runner `{program}` emitted no JSON-RPC response"),
        })?;
    let response: Value =
        serde_json::from_str(response_line).map_err(|error| PublicSeamError::InvalidPlan {
            message: format!(
                "configured stage runner `{program}` emitted invalid JSON-RPC response: {error}"
            ),
        })?;
    if let Some(error) = response.get("error") {
        return Err(PublicSeamError::InvalidPlan {
            message: format!("configured stage runner `{program}` returned error: {error}"),
        });
    }
    response
        .get("result")
        .cloned()
        .ok_or_else(|| PublicSeamError::InvalidPlan {
            message: format!("configured stage runner `{program}` response missing result"),
        })
}

fn stage_run_json_rpc_request(params: &Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": "leaven-seam-service-stage-run",
        "method": "leaven/stage.run",
        "params": params,
    })
}

fn stage_call_id(params: &Value) -> Result<&str, PublicSeamError> {
    params
        .get("payload")
        .and_then(|payload| payload.get("stage_call_id"))
        .and_then(Value::as_str)
        .ok_or_else(|| PublicSeamError::InvalidStageRun {
            message: "stage.run payload missing stage_call_id".to_owned(),
        })
}

fn stage_run_text_result(stage_call_id: &str, text: &str, summary: &str) -> Value {
    json!({
        "schema_version": "leaven.stage_run.v1",
        "message": "stage_run_result",
        "stage": "runner",
        "stage_call_id": stage_call_id,
        "output": {
            "kind": "text",
            "summary": summary,
            "value": text,
            "visibility": "optimizer_visible",
            "data_classes": ["candidate.output"]
        }
    })
}
