use std::{
    collections::BTreeMap,
    io::{BufRead, BufReader, Write},
    path::PathBuf,
    process::{Child, ChildStdin, ChildStdout, Command, ExitStatus, Stdio},
    sync::{Arc, Mutex, MutexGuard},
};

use leaven_public_seam::{
    AcpJsonRpcResponseDocument, AcpProfileDocument, AcpProgressDisposition, AcpProgressPriority,
    AcpSessionState, AcpStdioWorkerLaunch, AcpWorkerSession, PublicSeamError, PublicSeamPackage,
};
use serde_json::{Value, json};

pub const SESSION_UPDATE_METHOD: &str = "session/update";
pub const SESSION_CANCEL_METHOD: &str = "session/cancel";

pub type AcpTransportResult<T> = Result<T, AcpTransportError>;

#[derive(Debug, thiserror::Error)]
pub enum AcpTransportError {
    #[error(transparent)]
    PublicSeam(#[from] PublicSeamError),
    #[error("ACP stdio transport I/O failed while {action}")]
    Io {
        action: &'static str,
        #[source]
        source: std::io::Error,
    },
    #[error("ACP stdio transport JSON failed while {action}")]
    Json {
        action: &'static str,
        #[source]
        source: serde_json::Error,
    },
    #[error("ACP stdio worker exited before sending response for `{method}` request `{id}`")]
    WorkerExited { method: String, id: String },
    #[error("invalid ACP stdio protocol message: {message}")]
    Protocol { message: String },
    #[error("ACP stdio worker progress update was refused: {message}")]
    Backpressure { message: String },
    #[error("ACP stdio session cancelled by `{receipt}`: {reason}")]
    Cancelled { receipt: String, reason: String },
}

/// External worker process command for one ACP stdio session.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcpProcessCommand {
    program: PathBuf,
    args: Vec<String>,
    env: BTreeMap<String, String>,
    current_dir: Option<PathBuf>,
}

impl AcpProcessCommand {
    /// Creates a command from an executable path or name.
    pub fn new(program: impl Into<PathBuf>) -> Self {
        Self {
            program: program.into(),
            args: Vec::new(),
            env: BTreeMap::new(),
            current_dir: None,
        }
    }

    /// Adds one argument.
    #[must_use]
    pub fn arg(mut self, arg: impl Into<String>) -> Self {
        self.args.push(arg.into());
        self
    }

    /// Adds one environment value visible to the worker process.
    #[must_use]
    pub fn env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.insert(key.into(), value.into());
        self
    }

    /// Sets the process working directory.
    #[must_use]
    pub fn current_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.current_dir = Some(path.into());
        self
    }
}

/// Live ACP stdio session backed by a child process.
pub struct AcpStdioProcessSession {
    package: PublicSeamPackage,
    profile: AcpProfileDocument,
    session: Arc<Mutex<AcpWorkerSession>>,
    child: Child,
    stdin: Arc<Mutex<ChildStdin>>,
    stdout: BufReader<ChildStdout>,
    next_request: u64,
}

/// Cancellation handle that can interrupt a pending stdio extension call.
#[derive(Clone)]
pub struct AcpStdioCancellationHandle {
    session: Arc<Mutex<AcpWorkerSession>>,
    stdin: Arc<Mutex<ChildStdin>>,
}

impl AcpStdioProcessSession {
    /// Spawns an external worker process and binds it to the locked ACP profile.
    pub fn spawn(
        package: PublicSeamPackage,
        profile: AcpProfileDocument,
        command: AcpProcessCommand,
        bearer_token: impl Into<String>,
        endpoint: impl Into<String>,
        capability_fingerprint: impl Into<String>,
    ) -> AcpTransportResult<Self> {
        let AcpProcessCommand {
            program,
            args,
            env,
            current_dir,
        } = command;
        let session = AcpWorkerSession::start(&profile)?;
        let launch = AcpStdioWorkerLaunch::new(
            &profile,
            &session,
            bearer_token,
            endpoint,
            capability_fingerprint,
        )?;
        let mut process = Command::new(&program);
        process.args(&args);
        if let Some(current_dir) = &current_dir {
            process.current_dir(current_dir);
        }
        process.envs(&env);
        process.envs(launch.worker_env());
        process
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = process.spawn().map_err(|source| AcpTransportError::Io {
            action: "spawning ACP stdio worker",
            source,
        })?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| AcpTransportError::Protocol {
                message: "ACP stdio worker did not expose stdin".to_owned(),
            })?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| AcpTransportError::Protocol {
                message: "ACP stdio worker did not expose stdout".to_owned(),
            })?;
        Ok(Self {
            package,
            profile,
            session: Arc::new(Mutex::new(session)),
            child,
            stdin: Arc::new(Mutex::new(stdin)),
            stdout: BufReader::new(stdout),
            next_request: 0,
        })
    }

    /// Profile-derived session facts for the live worker process.
    pub fn worker_session_snapshot(&self) -> AcpWorkerSession {
        self.lock_session()
            .expect("session mutex is not poisoned")
            .clone()
    }

    /// Handle that can deliver ACP session cancellation while a call is in flight.
    #[must_use]
    pub fn cancellation_handle(&self) -> AcpStdioCancellationHandle {
        AcpStdioCancellationHandle {
            session: Arc::clone(&self.session),
            stdin: Arc::clone(&self.stdin),
        }
    }

    /// Sends one locked Leaven ACP extension request and waits for its response.
    pub fn call_extension(
        &mut self,
        method: &str,
        params: &Value,
    ) -> AcpTransportResult<AcpJsonRpcResponseDocument> {
        if self.lock_session()?.lifecycle().state() == AcpSessionState::Cancelled {
            return Err(AcpTransportError::Protocol {
                message: "ACP stdio session refuses extension calls after cancellation".to_owned(),
            });
        }
        let request_id = format!("leaven-acp-{}", self.next_request);
        self.next_request += 1;
        let request_value = json!({
            "jsonrpc": "2.0",
            "id": request_id,
            "method": method,
            "params": params
        });
        let request = self
            .package
            .validate_acp_jsonrpc_request_document(&self.profile, &request_value)?;
        self.write_message(&request_value)?;

        loop {
            let value = self.read_message(method, request.id())?;
            if self.handle_session_update(&value)?.is_some() {
                continue;
            }
            if let Some(cancellation) = self.cancellation_snapshot()? {
                return Err(AcpTransportError::Cancelled {
                    receipt: cancellation.receipt,
                    reason: cancellation.reason,
                });
            }
            return self
                .package
                .validate_acp_jsonrpc_response_document(&request, &value)
                .map_err(AcpTransportError::from);
        }
    }

    /// Sends ACP session cancellation to the live worker and records auditable lifecycle facts.
    pub fn cancel_with_error(
        &mut self,
        reason: impl Into<String>,
        receipt: impl Into<String>,
        error: Value,
    ) -> AcpTransportResult<()> {
        let cancellation = self
            .lock_session()?
            .lifecycle_mut()
            .cancel_with_error(reason, receipt, error)
            .map(cancellation_parts)?;
        self.write_cancellation(&cancellation)
    }

    /// Reads and applies one ACP session progress update without waiting for an extension response.
    pub fn read_next_session_update(&mut self) -> AcpTransportResult<AcpProgressDisposition> {
        let value = self.read_message(SESSION_UPDATE_METHOD, "notification")?;
        self.handle_session_update(&value)?
            .ok_or_else(|| AcpTransportError::Protocol {
                message: "expected ACP session update notification".to_owned(),
            })
    }

    /// Waits for the worker process to exit.
    pub fn wait_for_exit(&mut self) -> AcpTransportResult<ExitStatus> {
        self.child.wait().map_err(|source| AcpTransportError::Io {
            action: "waiting for ACP stdio worker",
            source,
        })
    }

    fn write_message(&self, value: &Value) -> AcpTransportResult<()> {
        let mut stdin = self.lock_stdin()?;
        write_json_line(&mut stdin, value)
    }

    fn read_message(&mut self, method: &str, id: &str) -> AcpTransportResult<Value> {
        let mut line = String::new();
        let count = self
            .stdout
            .read_line(&mut line)
            .map_err(|source| AcpTransportError::Io {
                action: "reading ACP stdio JSON-RPC line",
                source,
            })?;
        if count == 0 {
            if let Some(cancellation) = self.cancellation_snapshot()? {
                return Err(AcpTransportError::Cancelled {
                    receipt: cancellation.receipt,
                    reason: cancellation.reason,
                });
            }
            return Err(AcpTransportError::WorkerExited {
                method: method.to_owned(),
                id: id.to_owned(),
            });
        }
        serde_json::from_str(&line).map_err(|source| AcpTransportError::Json {
            action: "decoding ACP stdio JSON-RPC line",
            source,
        })
    }

    fn handle_session_update(
        &self,
        value: &Value,
    ) -> AcpTransportResult<Option<AcpProgressDisposition>> {
        let Some(object) = value.as_object() else {
            return Err(AcpTransportError::Protocol {
                message: "ACP stdio message must be an object".to_owned(),
            });
        };
        if object.get("method").and_then(Value::as_str) != Some(SESSION_UPDATE_METHOD) {
            return Ok(None);
        }
        if object.contains_key("id")
            || object.contains_key("result")
            || object.contains_key("error")
        {
            return Err(AcpTransportError::Protocol {
                message: "ACP session update must be a notification".to_owned(),
            });
        }
        let params = object
            .get("params")
            .and_then(Value::as_object)
            .ok_or_else(|| AcpTransportError::Protocol {
                message: "ACP session update must carry params object".to_owned(),
            })?;
        let message = params
            .get("message")
            .and_then(Value::as_str)
            .ok_or_else(|| AcpTransportError::Protocol {
                message: "ACP session update must carry message".to_owned(),
            })?;
        let priority = match params.get("priority").and_then(Value::as_str) {
            Some("noncritical") => AcpProgressPriority::Noncritical,
            Some("critical") | None => AcpProgressPriority::Critical,
            Some(other) => {
                return Err(AcpTransportError::Protocol {
                    message: format!("unknown ACP session update priority `{other}`"),
                });
            }
        };
        let disposition = self
            .lock_session()?
            .lifecycle_mut()
            .offer_progress(message, priority);
        match disposition {
            Ok(
                disposition @ (AcpProgressDisposition::Enqueued(_)
                | AcpProgressDisposition::DroppedNoncritical),
            ) => Ok(Some(disposition)),
            Ok(disposition @ AcpProgressDisposition::Disconnected(_)) => {
                if let Some(cancellation) = self.cancellation_snapshot()? {
                    self.write_cancellation(&cancellation)?;
                }
                Ok(Some(disposition))
            }
            Err(error) => Err(AcpTransportError::Backpressure {
                message: error.to_string(),
            }),
        }
    }

    fn lock_session(&self) -> AcpTransportResult<MutexGuard<'_, AcpWorkerSession>> {
        self.session
            .lock()
            .map_err(|_| AcpTransportError::Protocol {
                message: "ACP stdio session lifecycle lock is poisoned".to_owned(),
            })
    }

    fn lock_stdin(&self) -> AcpTransportResult<MutexGuard<'_, ChildStdin>> {
        self.stdin.lock().map_err(|_| AcpTransportError::Protocol {
            message: "ACP stdio writer lock is poisoned".to_owned(),
        })
    }

    fn cancellation_snapshot(&self) -> AcpTransportResult<Option<CancellationParts>> {
        let session = self.lock_session()?;
        Ok(session.lifecycle().cancellation().map(cancellation_parts))
    }

    fn write_cancellation(&self, cancellation: &CancellationParts) -> AcpTransportResult<()> {
        let notification = cancellation_notification(cancellation);
        let mut stdin = self.lock_stdin()?;
        write_json_line(&mut stdin, &notification)
    }
}

impl AcpStdioCancellationHandle {
    /// Sends ACP cancellation to the worker while another thread waits for a response.
    pub fn cancel_with_error(
        &self,
        reason: impl Into<String>,
        receipt: impl Into<String>,
        error: Value,
    ) -> AcpTransportResult<()> {
        let cancellation = self
            .session
            .lock()
            .map_err(|_| AcpTransportError::Protocol {
                message: "ACP stdio session lifecycle lock is poisoned".to_owned(),
            })?
            .lifecycle_mut()
            .cancel_with_error(reason, receipt, error)
            .map(cancellation_parts)?;
        let notification = cancellation_notification(&cancellation);
        let mut stdin = self.stdin.lock().map_err(|_| AcpTransportError::Protocol {
            message: "ACP stdio writer lock is poisoned".to_owned(),
        })?;
        write_json_line(&mut stdin, &notification)
    }
}

#[derive(Clone, Debug)]
struct CancellationParts {
    reason: String,
    receipt: String,
    error: Value,
}

fn cancellation_parts(
    cancellation: &leaven_public_seam::AcpSessionCancellation,
) -> CancellationParts {
    CancellationParts {
        reason: cancellation.reason().to_owned(),
        receipt: cancellation.receipt().to_owned(),
        error: cancellation.error().clone(),
    }
}

fn cancellation_notification(cancellation: &CancellationParts) -> Value {
    json!({
        "jsonrpc": "2.0",
        "method": SESSION_CANCEL_METHOD,
        "params": {
            "reason": cancellation.reason.clone(),
            "receipt": cancellation.receipt.clone(),
            "error": cancellation.error.clone()
        }
    })
}

fn write_json_line(writer: &mut ChildStdin, value: &Value) -> AcpTransportResult<()> {
    serde_json::to_writer(&mut *writer, value).map_err(|source| AcpTransportError::Json {
        action: "encoding ACP stdio JSON-RPC line",
        source,
    })?;
    writer
        .write_all(b"\n")
        .and_then(|()| writer.flush())
        .map_err(|source| AcpTransportError::Io {
            action: "writing ACP stdio JSON-RPC line",
            source,
        })
}

impl Drop for AcpStdioProcessSession {
    fn drop(&mut self) {
        drop(self.child.kill());
    }
}
