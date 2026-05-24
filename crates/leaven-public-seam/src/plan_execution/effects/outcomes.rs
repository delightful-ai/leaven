/// Host outcome for a typed `agent_run` call.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanAgentRunOutcome {
    pub(super) status: String,
    pub(super) parsed: Option<Value>,
    pub(super) transcript_ref: Option<Value>,
    pub(super) commands: Vec<Value>,
    pub(super) data_classes: Vec<String>,
    pub(super) replayability: String,
    pub(super) runtime_fingerprint: String,
    pub(super) cost: Option<Value>,
}

/// Blob refs for one observed command inside a provider-neutral agent session.
///
/// These refs are supplied by the host after it persists the observed command
/// streams/files. The seam verifies the refs against the captured bytes before
/// they can appear in a public `agent_session` value.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AgentCommandOutputRefs {
    stdout_ref: Value,
    stderr_ref: Value,
    file_refs: BTreeMap<WorkspacePath, Value>,
}

impl AgentCommandOutputRefs {
    /// Creates refs for stdout and stderr captured by an agent command.
    #[must_use]
    pub fn new(stdout_ref: Value, stderr_ref: Value) -> Self {
        Self {
            stdout_ref,
            stderr_ref,
            file_refs: BTreeMap::new(),
        }
    }

    /// Attaches a persisted blob ref for one captured output file.
    #[must_use]
    pub fn with_output_file(mut self, path: WorkspacePath, blob_ref: Value) -> Self {
        self.file_refs.insert(path, blob_ref);
        self
    }
}

impl PlanAgentRunOutcome {
    /// Creates a completed agent session outcome.
    #[must_use]
    fn completed(runtime_fingerprint: impl Into<String>) -> Self {
        Self {
            status: "completed".to_owned(),
            parsed: None,
            transcript_ref: None,
            commands: Vec::new(),
            data_classes: vec!["public".to_owned()],
            replayability: "has_declared_external_effects".to_owned(),
            runtime_fingerprint: runtime_fingerprint.into(),
            cost: None,
        }
    }

    /// Creates an agent session outcome from provider-neutral agent evidence.
    ///
    /// Every command observed in the session must have stdout/stderr refs, and
    /// every captured command output file must have an exact path-matched blob
    /// ref. This prevents hosts from treating unbound stdout as a proposal or
    /// attaching unrelated blobs after the agent has run.
    pub fn from_agent_session_with_command_output_refs(
        session: leaven_kernel::Metered<leaven_agent::AgentSession>,
        runtime_fingerprint: leaven_kernel::Fingerprint,
        transcript_ref: Value,
        session_receipt: impl Into<String>,
        command_output_refs: impl IntoIterator<Item = AgentCommandOutputRefs>,
    ) -> Result<Self, PublicSeamError> {
        let leaven_kernel::Metered { value, cost } = session;
        let session_receipt = session_receipt.into();
        let command_output_refs = command_output_refs.into_iter().collect::<Vec<_>>();
        if command_output_refs.len() != value.commands.len() {
            return Err(invalid_call(format!(
                "agent session has {} commands but {} command output ref sets",
                value.commands.len(),
                command_output_refs.len()
            )));
        }
        let mut outcome = Self::completed(format!(
            "fp_runtime_sha256_{}",
            fingerprint_hex(runtime_fingerprint)
        ))
        .with_status(agent::agent_status_value(&value.status))
        .with_transcript_ref(transcript_ref)
        .with_cost(cost_value(&cost));
        let commands = value
            .commands
            .iter()
            .zip(command_output_refs)
            .enumerate()
            .map(|(index, (command, refs))| {
                outcome.command_value_with_output_refs(index, command, &session_receipt, refs)
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(outcome.with_commands(commands))
    }

    /// Attaches a transcript blob reference.
    #[must_use]
    fn with_transcript_ref(mut self, transcript_ref: Value) -> Self {
        extend_data_classes_from_blob_ref(&mut self.data_classes, &transcript_ref);
        self.transcript_ref = Some(transcript_ref);
        self
    }

    /// Attaches the parsed payload required by JSON-schema output contracts.
    #[must_use]
    pub fn with_parsed(mut self, parsed: Value) -> Self {
        self.parsed = Some(parsed);
        self
    }

    /// Attaches command audit records.
    #[must_use]
    fn with_commands(mut self, commands: impl IntoIterator<Item = Value>) -> Self {
        self.commands.clear();
        for command in commands {
            extend_data_classes_from_agent_command(&mut self.data_classes, &command);
            self.commands.push(command);
        }
        self
    }

    /// Attaches a cost object.
    #[must_use]
    fn with_cost(mut self, cost: Value) -> Self {
        self.cost = Some(cost);
        self
    }

    #[must_use]
    fn with_status(mut self, status: impl Into<String>) -> Self {
        self.status = status.into();
        self
    }

    fn command_value_with_output_refs(
        &mut self,
        index: usize,
        command: &leaven_agent::CommandRecord,
        receipt: &str,
        refs: AgentCommandOutputRefs,
    ) -> Result<Value, PublicSeamError> {
        if command.output.stdout.truncated {
            return Err(invalid_call(format!(
                "agent command {index} stdout capture is truncated and cannot be bound to a blob ref"
            )));
        }
        validate_stream_blob_ref(
            &refs.stdout_ref,
            &command.output.stdout.bytes,
            &format!("agent command {index} stdout"),
        )?;
        if command.output.stderr.truncated {
            return Err(invalid_call(format!(
                "agent command {index} stderr capture is truncated and cannot be bound to a blob ref"
            )));
        }
        validate_stream_blob_ref(
            &refs.stderr_ref,
            &command.output.stderr.bytes,
            &format!("agent command {index} stderr"),
        )?;
        extend_data_classes_from_blob_ref(&mut self.data_classes, &refs.stdout_ref);
        extend_data_classes_from_blob_ref(&mut self.data_classes, &refs.stderr_ref);

        let mut file_refs = refs.file_refs;
        for path in file_refs.keys() {
            if !command.output.output_files.contains_key(path) {
                return Err(invalid_call(format!(
                    "agent command {index} output file `{}` blob ref does not match a captured command output file",
                    path.as_str()
                )));
            }
        }

        let mut files = serde_json::Map::new();
        for (path, captured) in &command.output.output_files {
            if captured.truncated {
                return Err(invalid_call(format!(
                    "agent command {index} output file `{}` capture is truncated and cannot be bound to a blob ref",
                    path.as_str()
                )));
            }
            let blob_ref = file_refs.remove(path).ok_or_else(|| {
                invalid_call(format!(
                    "agent command {index} output file `{}` is missing a blob ref",
                    path.as_str()
                ))
            })?;
            validate_stream_blob_ref(
                &blob_ref,
                &captured.bytes,
                &format!("agent command {index} output file `{}`", path.as_str()),
            )?;
            extend_data_classes_from_blob_ref(&mut self.data_classes, &blob_ref);
            files.insert(path.as_str().to_owned(), blob_ref);
        }

        let mut value = agent::agent_command_value(command, receipt);
        let object = value
            .as_object_mut()
            .expect("agent command values are JSON objects");
        object.insert("stdout_ref".to_owned(), refs.stdout_ref);
        object.insert("stderr_ref".to_owned(), refs.stderr_ref);
        if !files.is_empty() {
            object.insert("files".to_owned(), Value::Object(files));
        }
        Ok(value)
    }
}

fn extend_data_classes_from_agent_command(data_classes: &mut Vec<String>, command: &Value) {
    let Some(command) = command.as_object() else {
        return;
    };
    if let Some(stdout_ref) = command.get("stdout_ref") {
        extend_data_classes_from_blob_ref(data_classes, stdout_ref);
    }
    if let Some(stderr_ref) = command.get("stderr_ref") {
        extend_data_classes_from_blob_ref(data_classes, stderr_ref);
    }
    if let Some(files) = command.get("files").and_then(Value::as_object) {
        for blob_ref in files.values() {
            extend_data_classes_from_blob_ref(data_classes, blob_ref);
        }
    }
}

/// Host outcome for a typed `sandbox_exec` call.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanSandboxExecOutcome {
    pub(super) status: String,
    pub(super) exit_code: Option<i64>,
    pub(super) stdout_ref: Option<Value>,
    pub(super) stderr_ref: Option<Value>,
    pub(super) files: BTreeMap<String, Value>,
    pub(super) data_classes: Vec<String>,
    pub(super) replayability: String,
    pub(super) runtime_fingerprint: String,
    pub(super) cost: Option<Value>,
}

impl PlanSandboxExecOutcome {
    /// Creates a completed sandbox execution outcome.
    #[must_use]
    pub fn completed(runtime_fingerprint: impl Into<String>) -> Self {
        Self {
            status: "completed".to_owned(),
            exit_code: Some(0),
            stdout_ref: None,
            stderr_ref: None,
            files: BTreeMap::new(),
            data_classes: vec!["public".to_owned()],
            replayability: "boundary_managed".to_owned(),
            runtime_fingerprint: runtime_fingerprint.into(),
            cost: None,
        }
    }

    /// Creates a sandbox outcome from provider-neutral workspace command output.
    pub fn from_command_output(
        output: leaven_kernel::Metered<CommandOutput>,
        runtime_fingerprint: leaven_kernel::Fingerprint,
        stdout_ref: Value,
        stderr_ref: Value,
    ) -> Result<Self, PublicSeamError> {
        Self::from_command_output_with_file_refs(
            output,
            runtime_fingerprint,
            stdout_ref,
            stderr_ref,
            std::iter::empty::<(WorkspacePath, Value)>(),
        )
    }

    /// Creates a sandbox outcome from command output plus blob refs for captured files.
    ///
    /// Every file captured by the backend-neutral command output must have a
    /// matching blob ref, and every supplied file blob ref must correspond to a
    /// captured workspace file. This keeps file artifacts bound to the command
    /// result instead of letting hosts attach unrelated blobs after execution.
    pub fn from_command_output_with_file_refs(
        output: leaven_kernel::Metered<CommandOutput>,
        runtime_fingerprint: leaven_kernel::Fingerprint,
        stdout_ref: Value,
        stderr_ref: Value,
        file_refs: impl IntoIterator<Item = (WorkspacePath, Value)>,
    ) -> Result<Self, PublicSeamError> {
        let leaven_kernel::Metered { value, cost } = output;
        validate_stream_blob_ref(&stdout_ref, &value.stdout.bytes, "sandbox stdout")?;
        validate_stream_blob_ref(&stderr_ref, &value.stderr.bytes, "sandbox stderr")?;
        let mut outcome = Self::completed(format!(
            "fp_runtime_sha256_{}",
            fingerprint_hex(runtime_fingerprint)
        ));
        outcome.exit_code = value.status.code.map(i64::from);
        outcome = outcome.with_stream_refs(stdout_ref, stderr_ref);

        let mut file_refs_by_path = BTreeMap::new();
        for (path, blob_ref) in file_refs {
            if file_refs_by_path.insert(path.clone(), blob_ref).is_some() {
                return Err(invalid_call(format!(
                    "sandbox output file `{}` has duplicate blob refs",
                    path.as_str()
                )));
            }
        }
        for path in file_refs_by_path.keys() {
            if !value.output_files.contains_key(path) {
                return Err(invalid_call(format!(
                    "sandbox output file `{}` blob ref does not match a captured command output file",
                    path.as_str()
                )));
            }
        }
        for (path, captured) in &value.output_files {
            if captured.truncated {
                return Err(invalid_call(format!(
                    "sandbox output file `{}` capture is truncated and cannot be bound to a blob ref",
                    path.as_str()
                )));
            }
            let blob_ref = file_refs_by_path.get(path).ok_or_else(|| {
                invalid_call(format!(
                    "sandbox output file `{}` is missing a blob ref",
                    path.as_str()
                ))
            })?;
            outcome = outcome.with_file_ref(path.as_str(), blob_ref.clone(), &captured.bytes)?;
        }

        Ok(outcome.with_cost(cost_value(&cost)))
    }

    /// Attaches stdout and stderr blob references.
    #[must_use]
    fn with_stream_refs(mut self, stdout_ref: Value, stderr_ref: Value) -> Self {
        extend_data_classes_from_blob_ref(&mut self.data_classes, &stdout_ref);
        extend_data_classes_from_blob_ref(&mut self.data_classes, &stderr_ref);
        self.stdout_ref = Some(stdout_ref);
        self.stderr_ref = Some(stderr_ref);
        self
    }

    /// Attaches a captured output file blob reference after binding its byte audit.
    fn with_file_ref(
        mut self,
        path: impl Into<String>,
        blob_ref: Value,
        bytes: impl AsRef<[u8]>,
    ) -> Result<Self, PublicSeamError> {
        let path = path.into();
        WorkspacePath::new(&path).map_err(|error| {
            invalid_call(format!(
                "sandbox output file path must be relative workspace path: {error}"
            ))
        })?;
        validate_stream_blob_ref(
            &blob_ref,
            bytes.as_ref(),
            &format!("sandbox output file `{path}`"),
        )?;
        extend_data_classes_from_blob_ref(&mut self.data_classes, &blob_ref);
        self.files.insert(path, blob_ref);
        Ok(self)
    }

    /// Attaches a cost object.
    #[must_use]
    pub fn with_cost(mut self, cost: Value) -> Self {
        self.cost = Some(cost);
        self
    }
}

fn validate_stream_blob_ref(
    blob_ref: &Value,
    bytes: &[u8],
    stream: &str,
) -> Result<(), PublicSeamError> {
    let object = blob_ref
        .as_object()
        .ok_or_else(|| invalid_call(format!("{stream} blob ref must be an object")))?;
    let declared_bytes = object
        .get("bytes")
        .and_then(Value::as_u64)
        .ok_or_else(|| invalid_call(format!("{stream} blob ref must carry bytes")))?;
    let actual_bytes = u64::try_from(bytes.len()).map_err(|_| {
        invalid_call(format!(
            "{stream} captured output is too large for public byte audit"
        ))
    })?;
    if declared_bytes != actual_bytes {
        return Err(invalid_call(format!(
            "{stream} blob ref bytes `{declared_bytes}` do not match captured output bytes `{actual_bytes}`"
        )));
    }
    let declared_sha = object
        .get("sha256")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid_call(format!("{stream} blob ref must carry sha256")))?;
    let actual_sha = lower_hex_sha256(bytes);
    if declared_sha != actual_sha {
        return Err(invalid_call(format!(
            "{stream} blob ref sha256 does not match captured output"
        )));
    }
    Ok(())
}

fn lower_hex_sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// Host outcome for a typed `workspace_materialize` call.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanWorkspaceMaterializeOutcome {
    pub(super) workspace: String,
    pub(super) workspace_ref: Value,
    pub(super) lifetime: String,
    pub(super) data_classes: Vec<String>,
    pub(super) replayability: String,
    pub(super) runtime_fingerprint: String,
}

impl PlanWorkspaceMaterializeOutcome {
    /// Creates a live workspace handle outcome.
    #[must_use]
    pub fn new(
        workspace: impl Into<String>,
        lifetime: impl Into<String>,
        runtime_fingerprint: impl Into<String>,
    ) -> Self {
        let workspace = workspace.into();
        Self {
            workspace_ref: Value::String(workspace.clone()),
            workspace,
            lifetime: lifetime.into(),
            data_classes: vec!["public".to_owned()],
            replayability: "boundary_managed".to_owned(),
            runtime_fingerprint: runtime_fingerprint.into(),
        }
    }

    #[must_use]
    pub fn with_workspace_object_ref(
        mut self,
        run: Option<impl Into<String>>,
        snapshot_fingerprint: Option<impl Into<String>>,
    ) -> Self {
        self.workspace_ref = workspace_ref_object(
            &self.workspace,
            run.map(Into::into),
            snapshot_fingerprint.map(Into::into),
        );
        self
    }
}

/// Host outcome for a typed `workspace_release` call.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanWorkspaceReleaseOutcome {
    pub(super) workspace: String,
    pub(super) workspace_ref: Value,
    pub(super) lifetime: String,
    pub(super) runtime_fingerprint: String,
}

impl PlanWorkspaceReleaseOutcome {
    /// Creates a workspace release outcome.
    #[must_use]
    pub fn new(
        workspace: impl Into<String>,
        lifetime: impl Into<String>,
        runtime_fingerprint: impl Into<String>,
    ) -> Self {
        let workspace = workspace.into();
        Self {
            workspace_ref: Value::String(workspace.clone()),
            workspace,
            lifetime: lifetime.into(),
            runtime_fingerprint: runtime_fingerprint.into(),
        }
    }

    #[must_use]
    pub fn with_workspace_object_ref(
        mut self,
        run: Option<impl Into<String>>,
        snapshot_fingerprint: Option<impl Into<String>>,
    ) -> Self {
        self.workspace_ref = workspace_ref_object(
            &self.workspace,
            run.map(Into::into),
            snapshot_fingerprint.map(Into::into),
        );
        self
    }
}

/// Lowered `emit_run_event` request passed to a plan execution host.
#[derive(Clone, Copy, Debug)]
pub struct PlanEmitRunEventRequest<'a> {
    pub(super) name: &'a str,
    pub(super) write: &'a Value,
    pub(super) deps: &'a BTreeMap<String, Value>,
    pub(super) dependency_data_classes: &'a BTreeSet<String>,
    pub(super) base_revision: &'a str,
}

impl<'a> PlanEmitRunEventRequest<'a> {
    /// Operation binding name.
    pub const fn name(&self) -> &'a str {
        self.name
    }

    /// Typed `emit_run_event` write body from the Plan IR.
    pub const fn write(&self) -> &'a Value {
        self.write
    }

    /// Resolved dependency bindings visible to this write.
    pub const fn deps(&self) -> &'a BTreeMap<String, Value> {
        self.deps
    }

    /// Data classes carried by dependency bindings but not necessarily present
    /// in the host-visible JSON values.
    pub const fn dependency_data_classes(&self) -> &'a BTreeSet<String> {
        self.dependency_data_classes
    }

    /// Base graph revision supplied by the public-seam execution context.
    pub const fn base_revision(&self) -> &'a str {
        self.base_revision
    }
}

/// Host outcome for a typed `emit_run_event` write.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlanEmitRunEventOutcome {
    pub(super) event_id: String,
    pub(super) committed_revision: String,
}

impl PlanEmitRunEventOutcome {
    /// Creates an emitted event outcome.
    pub fn new(event_id: impl Into<String>, committed_revision: impl Into<String>) -> Self {
        Self {
            event_id: event_id.into(),
            committed_revision: committed_revision.into(),
        }
    }
}
