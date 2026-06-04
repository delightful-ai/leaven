use std::{
    collections::BTreeMap,
    io::{BufRead, BufReader, Stdin, Stdout, Write, stdin, stdout},
    path::PathBuf,
    process::{Child, ChildStdin, ChildStdout, Command, ExitStatus, Stdio},
    sync::{Arc, Mutex, MutexGuard},
};

use leaven_public_seam::{
    AcpJsonRpcRequestDocument, AcpJsonRpcResponseDocument, AcpProfileDocument,
    AcpProgressDisposition, AcpProgressPriority, AcpSessionState, AcpStageRunResponseDocument,
    AcpStdioWorkerLaunch, AcpWorkerSession, LockedMethod, PublicSeamError, PublicSeamPackage,
};
use serde_json::{Value, json};

/// Classification of one inbound JSON-RPC line on the demultiplexing read loop.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum InboundLine {
    /// `session/update` lifecycle notification (no id, no result).
    SessionUpdate,
    /// Worker-initiated extension request (id + method + params, no result).
    WorkerRequest,
    /// Host→worker response keyed by the outstanding request id.
    HostResponse,
}

pub const SESSION_UPDATE_METHOD: &str = "session/update";
pub const SESSION_CANCEL_METHOD: &str = "session/cancel";

pub type AcpTransportResult<T> = Result<T, AcpTransportError>;

/// Host effect handler for worker-initiated ACP extension requests.
///
/// The worker is the ACP agent and the engine is the ACP client, so the worker
/// runs a stage and calls Leaven extension methods *back* into the engine. The
/// transport validates each inbound request's Plan IR params, hands them to this
/// host, and validates the host's extension result before writing it back. The
/// host owns no graph mutation, no transport framing, and no JSON-RPC ids; it
/// only lowers a validated request into a Leaven extension result envelope.
///
/// For the bidirectional spike only `lm_complete` is wired. Every other locked
/// method rejects through the default `service` dispatch until its host lowering
/// lands.
pub trait AcpEffectHost {
    /// Services a worker-initiated `leaven/lm.complete` request.
    ///
    /// `params` is the validated Plan IR document carried by the inbound
    /// request. The returned value must be a Leaven extension result envelope;
    /// the transport validates it and stamps the launched capability fingerprint
    /// before writing it back to the worker.
    fn lm_complete(&self, params: &Value) -> AcpTransportResult<Value>;

    /// Dispatches one validated inbound request to its host lowering.
    ///
    /// The default routes `leaven/lm.complete` to [`AcpEffectHost::lm_complete`]
    /// and rejects every other locked method as unimplemented for this slice.
    fn service(&self, method: LockedMethod, params: &Value) -> AcpTransportResult<Value> {
        match method {
            LockedMethod::LmComplete => self.lm_complete(params),
            other => Err(AcpTransportError::EffectUnimplemented {
                method: other.as_str().to_owned(),
            }),
        }
    }
}

/// Effect host that refuses every worker-initiated request.
///
/// Host→worker `call_extension` callers that do not expect worker callbacks pass
/// this so the single demultiplexing read loop still has a host to dispatch to.
/// If a worker unexpectedly initiates a request, the demux rejects it instead of
/// silently mishandling the line.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RejectAllEffectHost;

impl AcpEffectHost for RejectAllEffectHost {
    fn lm_complete(&self, _params: &Value) -> AcpTransportResult<Value> {
        Err(AcpTransportError::EffectUnimplemented {
            method: "leaven/lm.complete".to_owned(),
        })
    }
}

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
    #[error("ACP effect host has no lowering for worker-initiated method `{method}`")]
    EffectUnimplemented { method: String },
    #[error(
        "ACP effect host result for `{method}` carries capability fingerprint `{actual}`, not the launched session fingerprint `{expected}`"
    )]
    EffectFingerprintMismatch {
        method: String,
        expected: String,
        actual: String,
    },
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

/// Generic ACP stdio session over one line-framed JSON-RPC reader/writer pair.
///
/// This is the demultiplexing transport core: it owns the locked profile binding,
/// the worker session lifecycle, the capability fingerprint, the read loop that
/// classifies every inbound line, the host→worker `call_extension`/
/// `dispatch_stage_run` legs, and the worker→host effect-callback servicing. It is
/// agnostic to where the bytes come from. [`AcpStdioProcessSession`] specializes it
/// over a spawned child's stdin/stdout; [`AcpStdioInheritedSession`] specializes it
/// over the process's own inherited stdin/stdout, so `leaven serve --stdio` runs
/// the same client loop against its parent without spawning a child.
pub struct AcpStdioSession<R: BufRead, W: Write> {
    package: PublicSeamPackage,
    profile: AcpProfileDocument,
    session: Arc<Mutex<AcpWorkerSession>>,
    capability_fingerprint: String,
    stdin: Arc<Mutex<W>>,
    stdout: R,
    next_request: u64,
}

/// Live ACP stdio session backed by a child process.
///
/// The Leaven engine is the ACP client; this session spawns and owns the external
/// worker (the ACP agent) and shares the demultiplexing transport core
/// [`AcpStdioSession`] over the child's piped stdin/stdout.
pub struct AcpStdioProcessSession {
    core: AcpStdioSession<BufReader<ChildStdout>, ChildStdin>,
    child: Child,
}

/// ACP stdio session running the client loop over the process's own stdio.
///
/// This is the inverse spawn direction of [`AcpStdioProcessSession`]: the Leaven
/// engine is still the ACP client driving the demultiplexing transport core, but
/// its parent spawned *it* and passed the locked capability env. The session reads
/// the parent's JSON-RPC over inherited stdin and writes its own dispatches to
/// inherited stdout — no child process is spawned. `leaven serve --stdio` runs
/// here.
pub struct AcpStdioInheritedSession {
    core: AcpStdioSession<BufReader<Stdin>, Stdout>,
}

/// Cancellation handle that can interrupt a pending stdio extension call.
#[derive(Clone)]
pub struct AcpStdioCancellationHandle<W: Write> {
    session: Arc<Mutex<AcpWorkerSession>>,
    stdin: Arc<Mutex<W>>,
}

impl<R: BufRead, W: Write> AcpStdioSession<R, W> {
    /// Binds a reader/writer pair to the locked ACP profile and session lifecycle.
    ///
    /// `capability_fingerprint` is the launched session fingerprint the transport
    /// stamps onto every worker→host effect reply. Callers that spawn a child use
    /// [`AcpStdioProcessSession::spawn`]; callers that inherit the process stdio use
    /// [`AcpStdioInheritedSession::bind`].
    fn new(
        package: PublicSeamPackage,
        profile: AcpProfileDocument,
        session: AcpWorkerSession,
        capability_fingerprint: String,
        stdin: W,
        stdout: R,
    ) -> Self {
        Self {
            package,
            profile,
            session: Arc::new(Mutex::new(session)),
            capability_fingerprint,
            stdin: Arc::new(Mutex::new(stdin)),
            stdout,
            next_request: 0,
        }
    }

    /// Profile-derived session facts for the live worker process.
    pub fn worker_session_snapshot(&self) -> AcpWorkerSession {
        self.lock_session()
            .expect("session mutex is not poisoned")
            .clone()
    }

    /// Handle that can deliver ACP session cancellation while a call is in flight.
    #[must_use]
    pub fn cancellation_handle(&self) -> AcpStdioCancellationHandle<W> {
        AcpStdioCancellationHandle {
            session: Arc::clone(&self.session),
            stdin: Arc::clone(&self.stdin),
        }
    }

    /// Sends one locked Leaven ACP extension request and waits for its response.
    ///
    /// While waiting, the demultiplexing read loop also services any
    /// worker-initiated extension requests (worker→host effect callbacks)
    /// through `host`, replying before this call's response arrives. Host→worker
    /// callers that expect no callbacks pass [`RejectAllEffectHost`].
    pub fn call_extension(
        &mut self,
        method: LockedMethod,
        params: &Value,
        host: &impl AcpEffectHost,
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
            "method": method.as_str(),
            "params": params
        });
        let request = self
            .package
            .validate_acp_jsonrpc_request_document(&self.profile, &request_value)?;
        self.write_message(&request_value)?;

        loop {
            let (value, line) = self.read_until_actionable(method.as_str(), request.id())?;
            if line == InboundLine::WorkerRequest {
                // Service the worker→host effect callback and keep waiting for
                // this call's own response.
                self.service_inbound_request(&value, host)?;
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

    /// Sends one `leaven/stage.run` dispatch and waits for its stage-run result.
    ///
    /// This is the host→worker stage-dispatch leg of the locked profile: the
    /// engine tells the worker to run one stage, carrying a role-scoped stage-run
    /// request (not Plan IR). While waiting, the demultiplexing read loop services
    /// the worker's `leaven/lm.complete` (and later other) effect callbacks
    /// through `host`, replying before this dispatch's result arrives. The result
    /// is validated as a locked stage-run result, so a worker cannot answer a
    /// stage dispatch with a Plan Result or a shapeless payload.
    pub fn dispatch_stage_run(
        &mut self,
        stage_run_request: &Value,
        host: &impl AcpEffectHost,
    ) -> AcpTransportResult<AcpStageRunResponseDocument> {
        if self.lock_session()?.lifecycle().state() == AcpSessionState::Cancelled {
            return Err(AcpTransportError::Protocol {
                message: "ACP stdio session refuses stage dispatch after cancellation".to_owned(),
            });
        }
        let request_id = format!("leaven-acp-{}", self.next_request);
        self.next_request += 1;
        let request_value = json!({
            "jsonrpc": "2.0",
            "id": request_id,
            "method": "leaven/stage.run",
            "params": stage_run_request
        });
        let request = self
            .package
            .validate_acp_stage_run_request_document(&self.profile, &request_value)?;
        self.write_message(&request_value)?;

        loop {
            let (value, line) = self.read_until_actionable("leaven/stage.run", request.id())?;
            if line == InboundLine::WorkerRequest {
                self.service_inbound_request(&value, host)?;
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
                .validate_acp_stage_run_response_document(&request, &value)
                .map_err(AcpTransportError::from);
        }
    }

    /// Reads one inbound line and services a worker-initiated extension request.
    ///
    /// This is the worker→host leg of the bidirectional seam: the worker is the
    /// ACP agent and initiates `leaven/lm.complete` (and, later, other effect
    /// callbacks) back into the engine. The transport validates the inbound Plan
    /// IR params, dispatches to `host`, validates the host's extension result,
    /// stamps it with the launched capability fingerprint, and writes it back
    /// under the worker's request id. Session updates that precede the request
    /// are applied as lifecycle control without ending the call.
    pub fn serve_next_inbound_request(
        &mut self,
        host: &impl AcpEffectHost,
    ) -> AcpTransportResult<AcpJsonRpcRequestDocument> {
        if self.lock_session()?.lifecycle().state() == AcpSessionState::Cancelled {
            return Err(AcpTransportError::Protocol {
                message: "ACP stdio session refuses inbound requests after cancellation".to_owned(),
            });
        }
        let (value, line) = self.read_until_actionable("worker_inbound_request", "inbound")?;
        match line {
            InboundLine::WorkerRequest => self.service_inbound_request(&value, host),
            InboundLine::HostResponse => Err(AcpTransportError::Protocol {
                message: "ACP worker sent a response while the host expected a request".to_owned(),
            }),
            InboundLine::SessionUpdate => unreachable!("read_until_actionable filters updates"),
        }
    }

    /// Reads inbound lines, applying `session/update` notifications as lifecycle
    /// control, until one classifies as a worker request or host response.
    fn read_until_actionable(
        &mut self,
        method: &str,
        id: &str,
    ) -> AcpTransportResult<(Value, InboundLine)> {
        loop {
            let value = self.read_message(method, id)?;
            match self.classify_inbound(&value)? {
                InboundLine::SessionUpdate => {}
                actionable => return Ok((value, actionable)),
            }
        }
    }

    fn classify_inbound(&self, value: &Value) -> AcpTransportResult<InboundLine> {
        if self.handle_session_update(value)?.is_some() {
            return Ok(InboundLine::SessionUpdate);
        }
        let object = value
            .as_object()
            .ok_or_else(|| AcpTransportError::Protocol {
                message: "ACP stdio message must be an object".to_owned(),
            })?;
        // A worker-initiated request carries a method and no result/error; a
        // host→worker response carries a result/error and no method.
        if object.contains_key("method")
            && !object.contains_key("result")
            && !object.contains_key("error")
        {
            return Ok(InboundLine::WorkerRequest);
        }
        Ok(InboundLine::HostResponse)
    }

    fn service_inbound_request(
        &self,
        value: &Value,
        host: &impl AcpEffectHost,
    ) -> AcpTransportResult<AcpJsonRpcRequestDocument> {
        // Validate the worker-initiated request as locked Plan IR and gate the
        // method through the profile, rejecting private/MCP inbound exactly as
        // the host→worker direction does.
        let request = self
            .package
            .validate_acp_jsonrpc_request_document(&self.profile, value)?;
        let params = value
            .get("params")
            .expect("validated inbound request carries Plan IR params");
        let result = host.service(request.method(), params)?;
        let result = self.stamp_session_fingerprint(request.method(), result)?;
        // Validate the host's extension result before it crosses the boundary.
        self.package
            .validate_acp_extension_result_document(&result)?;
        let response = json!({
            "jsonrpc": "2.0",
            "id": request.id(),
            "result": result
        });
        self.write_message(&response)?;
        Ok(request)
    }

    /// Binds the host effect result to the launched session by enforcing its
    /// capability fingerprint. The host stamps a fingerprint only if it left the
    /// field absent; a divergent fingerprint is rejected so a host lowering can
    /// never answer on behalf of a different session.
    fn stamp_session_fingerprint(
        &self,
        method: LockedMethod,
        mut result: Value,
    ) -> AcpTransportResult<Value> {
        let object = result
            .as_object_mut()
            .ok_or_else(|| AcpTransportError::Protocol {
                message: "ACP effect host result must be an object".to_owned(),
            })?;
        match object.get("capability_fingerprint").and_then(Value::as_str) {
            None => {
                object.insert(
                    "capability_fingerprint".to_owned(),
                    json!(self.capability_fingerprint),
                );
                Ok(result)
            }
            Some(actual) if actual == self.capability_fingerprint => Ok(result),
            Some(actual) => Err(AcpTransportError::EffectFingerprintMismatch {
                method: method.as_str().to_owned(),
                expected: self.capability_fingerprint.clone(),
                actual: actual.to_owned(),
            }),
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

    fn write_message(&self, value: &Value) -> AcpTransportResult<()> {
        let mut stdin = self.lock_stdin()?;
        write_json_line(&mut *stdin, value)
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

    fn lock_stdin(&self) -> AcpTransportResult<MutexGuard<'_, W>> {
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
        write_json_line(&mut *stdin, &notification)
    }
}

impl<W: Write> AcpStdioCancellationHandle<W> {
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
        write_json_line(&mut *stdin, &notification)
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

fn write_json_line<W: Write>(writer: &mut W, value: &Value) -> AcpTransportResult<()> {
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

impl AcpStdioProcessSession {
    /// Spawns an external worker process and binds it to the locked ACP profile.
    ///
    /// The Leaven engine is the ACP client: it spawns the worker (the ACP agent),
    /// injects the locked capability env, and drives the demultiplexing transport
    /// core over the child's piped stdin/stdout.
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
        let capability_fingerprint = capability_fingerprint.into();
        let launch = AcpStdioWorkerLaunch::new(
            &profile,
            &session,
            bearer_token,
            endpoint,
            capability_fingerprint.clone(),
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
        let core = AcpStdioSession::new(
            package,
            profile,
            session,
            capability_fingerprint,
            stdin,
            BufReader::new(stdout),
        );
        Ok(Self { core, child })
    }

    /// The demultiplexing transport core driving this child-process session.
    ///
    /// Callers reuse the shared transport legs (`call_extension`,
    /// `dispatch_stage_run`, `serve_next_inbound_request`, cancellation, session
    /// updates) through the core, so a host loop written against
    /// [`AcpStdioSession`] runs unchanged over a spawned child or inherited stdio.
    pub fn session_mut(&mut self) -> &mut AcpStdioSession<BufReader<ChildStdout>, ChildStdin> {
        &mut self.core
    }

    /// Profile-derived session facts for the live worker process.
    pub fn worker_session_snapshot(&self) -> AcpWorkerSession {
        self.core.worker_session_snapshot()
    }

    /// Handle that can deliver ACP session cancellation while a call is in flight.
    #[must_use]
    pub fn cancellation_handle(&self) -> AcpStdioCancellationHandle<ChildStdin> {
        self.core.cancellation_handle()
    }

    /// Sends one locked Leaven ACP extension request and waits for its response.
    pub fn call_extension(
        &mut self,
        method: LockedMethod,
        params: &Value,
        host: &impl AcpEffectHost,
    ) -> AcpTransportResult<AcpJsonRpcResponseDocument> {
        self.core.call_extension(method, params, host)
    }

    /// Sends one `leaven/stage.run` dispatch and waits for its stage-run result.
    pub fn dispatch_stage_run(
        &mut self,
        stage_run_request: &Value,
        host: &impl AcpEffectHost,
    ) -> AcpTransportResult<AcpStageRunResponseDocument> {
        self.core.dispatch_stage_run(stage_run_request, host)
    }

    /// Reads one inbound line and services a worker-initiated extension request.
    pub fn serve_next_inbound_request(
        &mut self,
        host: &impl AcpEffectHost,
    ) -> AcpTransportResult<AcpJsonRpcRequestDocument> {
        self.core.serve_next_inbound_request(host)
    }

    /// Sends ACP session cancellation to the live worker and records lifecycle facts.
    pub fn cancel_with_error(
        &mut self,
        reason: impl Into<String>,
        receipt: impl Into<String>,
        error: Value,
    ) -> AcpTransportResult<()> {
        self.core.cancel_with_error(reason, receipt, error)
    }

    /// Reads and applies one ACP session progress update.
    pub fn read_next_session_update(&mut self) -> AcpTransportResult<AcpProgressDisposition> {
        self.core.read_next_session_update()
    }

    /// Waits for the worker process to exit.
    pub fn wait_for_exit(&mut self) -> AcpTransportResult<ExitStatus> {
        self.child.wait().map_err(|source| AcpTransportError::Io {
            action: "waiting for ACP stdio worker",
            source,
        })
    }
}

impl Drop for AcpStdioProcessSession {
    fn drop(&mut self) {
        drop(self.child.kill());
    }
}

impl AcpStdioInheritedSession {
    /// Binds the process's own inherited stdin/stdout to the locked ACP profile.
    ///
    /// This is the inverse spawn direction: the parent already spawned this process
    /// (for example `leaven serve --stdio`) and injected the locked capability env,
    /// so there is no child to launch. The engine is still the ACP client driving
    /// the demultiplexing core; it dispatches `leaven/stage.run` to the parent over
    /// inherited stdout and services the parent's `leaven/lm.complete` callbacks
    /// from inherited stdin. The launch facts (token/endpoint/fingerprint) are
    /// validated to honor the same launch contract a spawned worker receives.
    pub fn bind(
        package: PublicSeamPackage,
        profile: AcpProfileDocument,
        bearer_token: impl Into<String>,
        endpoint: impl Into<String>,
        capability_fingerprint: impl Into<String>,
    ) -> AcpTransportResult<Self> {
        let session = AcpWorkerSession::start(&profile)?;
        let capability_fingerprint = capability_fingerprint.into();
        // Validate the launch contract (non-empty token/endpoint/fingerprint,
        // stdio transport) exactly as a spawned worker launch does, even though
        // the env was injected by the parent rather than projected by this process.
        AcpStdioWorkerLaunch::new(
            &profile,
            &session,
            bearer_token,
            endpoint,
            capability_fingerprint.clone(),
        )?;
        let core = AcpStdioSession::new(
            package,
            profile,
            session,
            capability_fingerprint,
            stdout(),
            BufReader::new(stdin()),
        );
        Ok(Self { core })
    }

    /// The demultiplexing transport core driving this inherited-stdio session.
    pub fn session_mut(&mut self) -> &mut AcpStdioSession<BufReader<Stdin>, Stdout> {
        &mut self.core
    }
}
