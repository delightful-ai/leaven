use std::io::{BufRead, BufReader, Read, Write};
use std::process::{ChildStdin, Command, Stdio};

use leaven_public_seam::{LockedMethod, PublicSeamError};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

const STDERR_CAPTURE_LIMIT: usize = 64 * 1024;

/// Configured runner-stage execution for public-seam service processes.
#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SeamStageConfig {
    /// No stage runner is wired.
    #[default]
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
    pub(crate) fn runner_result(
        &self,
        params: &Value,
        effects: &mut impl FnMut(LockedMethod, &Value) -> Result<Value, PublicSeamError>,
    ) -> Result<Value, PublicSeamError> {
        match self {
            Self::None => Err(PublicSeamError::InvalidPlan {
                message: "configured seam service does not provide a stage runner".to_owned(),
            }),
            Self::MockRunner { text, summary } => {
                Ok(stage_run_text_result(stage_call_id(params)?, text, summary))
            }
            Self::CommandRunner { argv } => command_runner_result(argv, params, effects),
        }
    }
}

/// Dispatch one `leaven/stage.run` request to a configured subprocess worker.
///
/// `params` is the full stage-run params object (`message`/`stage`/`payload`).
/// Worker-initiated nested callbacks are serviced through `effects` while the
/// stage is active; the caller scopes `effects` (for example, to refuse
/// `case.target` reads during runner-stage dispatch). This is the same machinery
/// the configured runner-stage handler uses, reused by the optimize-run host so
/// runner and scorer dispatch share one transport path.
pub fn command_runner_result(
    argv: &[String],
    params: &Value,
    effects: &mut impl FnMut(LockedMethod, &Value) -> Result<Value, PublicSeamError>,
) -> Result<Value, PublicSeamError> {
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
    let mut stdin = child
        .stdin
        .take()
        .ok_or_else(|| PublicSeamError::InvalidPlan {
            message: "configured stage runner stdin was not piped".to_owned(),
        })?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| PublicSeamError::InvalidPlan {
            message: "configured stage runner stdout was not piped".to_owned(),
        })?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| PublicSeamError::InvalidPlan {
            message: "configured stage runner stderr was not piped".to_owned(),
        })?;
    let stderr_drain = std::thread::spawn(move || read_stderr(stderr));
    let interaction = stdin
        .write_all(&request_line)
        .and_then(|()| stdin.write_all(b"\n"))
        .map_err(|error| PublicSeamError::InvalidPlan {
            message: format!("failed to write stage.run request to `{program}`: {error}"),
        })
        .and_then(|()| read_command_runner_messages(program, stdout, &mut stdin, effects));
    if interaction.is_err() {
        let _ = child.kill();
    }
    drop(stdin);
    let status = child.wait();
    let stderr = stderr_drain.join();

    let result = interaction?;
    let status = status.map_err(|error| PublicSeamError::InvalidPlan {
        message: format!("failed waiting for configured stage runner `{program}`: {error}"),
    })?;
    let stderr = stderr.map_err(|_| PublicSeamError::InvalidPlan {
        message: format!("failed collecting configured stage runner `{program}` stderr"),
    })?;
    if !status.success() {
        return Err(PublicSeamError::InvalidPlan {
            message: format!("configured stage runner `{program}` exited with {status}: {stderr}"),
        });
    }
    Ok(result)
}

fn read_command_runner_messages(
    program: &str,
    stdout: impl Read,
    stdin: &mut ChildStdin,
    effects: &mut impl FnMut(LockedMethod, &Value) -> Result<Value, PublicSeamError>,
) -> Result<Value, PublicSeamError> {
    let mut reader = BufReader::new(stdout);
    let mut line = String::new();
    loop {
        line.clear();
        let bytes = reader
            .read_line(&mut line)
            .map_err(|error| PublicSeamError::InvalidPlan {
                message: format!("failed reading configured stage runner `{program}`: {error}"),
            })?;
        if bytes == 0 {
            return Err(PublicSeamError::InvalidPlan {
                message: format!(
                    "configured stage runner `{program}` closed stdout before stage result"
                ),
            });
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let message: Value =
            serde_json::from_str(trimmed).map_err(|error| PublicSeamError::InvalidPlan {
                message: format!(
                    "configured stage runner `{program}` emitted invalid JSON-RPC message: {error}"
                ),
            })?;
        if let Some(method_name) = message.get("method").and_then(Value::as_str) {
            let method =
                LockedMethod::parse(method_name).ok_or_else(|| PublicSeamError::InvalidPlan {
                    message: format!(
                        "configured stage runner `{program}` requested unknown Leaven method `{method_name}`"
                    ),
                })?;
            let id = message.get("id").cloned().unwrap_or(Value::Null);
            let params = message
                .get("params")
                .ok_or_else(|| PublicSeamError::InvalidPlan {
                    message: format!(
                        "configured stage runner `{program}` request `{method_name}` missing params"
                    ),
                })?;
            let response = match effects(method, params) {
                Ok(result) => json!({"jsonrpc": "2.0", "id": id, "result": result}),
                Err(error) => json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "error": {
                        "code": -32000,
                        "message": error.to_string()
                    }
                }),
            };
            write_json_line(stdin, &response, program)?;
            continue;
        }
        if let Some(error) = message.get("error") {
            return Err(PublicSeamError::InvalidPlan {
                message: format!("configured stage runner `{program}` returned error: {error}"),
            });
        }
        return message
            .get("result")
            .cloned()
            .ok_or_else(|| PublicSeamError::InvalidPlan {
                message: format!("configured stage runner `{program}` response missing result"),
            });
    }
}

fn write_json_line(
    stdin: &mut ChildStdin,
    value: &Value,
    program: &str,
) -> Result<(), PublicSeamError> {
    let bytes = serde_json::to_vec(value).map_err(|error| PublicSeamError::InvalidPlan {
        message: format!("failed to serialize stage worker callback response: {error}"),
    })?;
    stdin
        .write_all(&bytes)
        .and_then(|()| stdin.write_all(b"\n"))
        .map_err(|error| PublicSeamError::InvalidPlan {
            message: format!("failed writing callback response to `{program}`: {error}"),
        })
}

fn read_stderr(mut stderr: impl Read) -> String {
    let mut captured = Vec::new();
    let mut buffer = [0_u8; 8 * 1024];
    let mut truncated = false;
    while let Ok(bytes_read) = stderr.read(&mut buffer) {
        if bytes_read == 0 {
            break;
        }
        let remaining = STDERR_CAPTURE_LIMIT.saturating_sub(captured.len());
        let captured_bytes = bytes_read.min(remaining);
        captured.extend_from_slice(&buffer[..captured_bytes]);
        truncated |= captured_bytes < bytes_read;
    }
    let mut text = String::from_utf8_lossy(&captured).into_owned();
    if truncated {
        text.push_str("\n[stderr truncated]");
    }
    text.trim().to_owned()
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

#[cfg(all(test, unix))]
mod tests {
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;

    use leaven_public_seam::LockedMethod;
    use serde_json::{Value, json};

    use super::command_runner_result;

    #[test]
    fn command_runner_drains_stderr_while_waiting_for_result() {
        let worker = noisy_worker();
        let result = command_runner_result(
            &[worker.display().to_string()],
            &json!({"payload": {"stage_call_id": "sc_noisy"}}),
            &mut |_method: LockedMethod, _params: &Value| {
                panic!("noisy worker does not issue callbacks")
            },
        )
        .unwrap();

        assert_eq!(result, json!({"worker": "completed"}));
    }

    fn noisy_worker() -> PathBuf {
        let directory = tempfile::tempdir().unwrap().keep();
        let path = directory.join("noisy-stage-worker");
        std::fs::write(
            &path,
            r#"#!/usr/bin/env python3
import json
import sys

json.loads(sys.stdin.readline())
sys.stderr.write("x" * 1_000_000)
sys.stderr.flush()
print(json.dumps({
    "jsonrpc": "2.0",
    "id": "leaven-seam-service-stage-run",
    "result": {"worker": "completed"},
}))
"#,
        )
        .unwrap();
        let mut permissions = std::fs::metadata(&path).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&path, permissions).unwrap();
        path
    }
}
