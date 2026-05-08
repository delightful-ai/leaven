use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use futures::future::{BoxFuture, FutureExt};
use leaven_agent::{
    AgentInstructions, AgentRunContext, AgentRunRequest, AgentRuntime, AgentRuntimeCapabilities,
    AgentRuntimeError, CancellationRef, FakeAgentAction, FakeAgentRuntime, OutputContract,
    TranscriptEvent, TranscriptRole, WorkspaceAccessMode,
};
use leaven_kernel::{AgentRuntimeId, AgentSessionId, BudgetSnapshot, Cost, Fingerprint};
use leaven_workspace::{
    Command, CommandOutput, ExitStatus, Workspace, WorkspaceBackend, WorkspaceError, WorkspacePath,
    WorkspaceView,
};

#[test]
fn fake_runtime_executes_actions_and_validates_file_contract() {
    futures::executor::block_on(async {
        let mut workspace = memory_workspace();
        let mut view = workspace.view();
        let output_path = WorkspacePath::new("output/result.json").unwrap();
        let runtime = FakeAgentRuntime::new(vec![
            FakeAgentAction::AssistantMessage("done".to_owned()),
            FakeAgentAction::WriteFile {
                path: output_path.clone(),
                bytes: br#"{"ok":true}"#.to_vec(),
            },
            FakeAgentAction::RawProviderEvent {
                kind: "turn".to_owned(),
                payload: "1".to_owned(),
            },
        ])
        .with_cost(Cost::llm_calls(1));
        let request = AgentRunRequest::new(
            AgentInstructions::task("write the result"),
            OutputContract::JsonFile {
                path: output_path.clone(),
                schema: None,
            },
        );

        let metered = runtime
            .run_session(
                &mut view,
                request,
                AgentRunContext::new(AgentSessionId::new(), &BudgetSnapshot::default()),
            )
            .await
            .unwrap();

        assert_eq!(metered.cost, Cost::llm_calls(1));
        assert_eq!(metered.value.output_files, vec![output_path.clone()]);
        assert_eq!(metered.value.raw_provider_events.len(), 1);
        assert_eq!(view.read_file(&output_path).unwrap(), br#"{"ok":true}"#);
        assert!(metered.value.transcript.events.iter().any(|event| {
            matches!(
                event,
                TranscriptEvent::Message {
                    role: TranscriptRole::Assistant,
                    content,
                } if content == "done"
            )
        }));

        drop(view);
        workspace.cleanup().await.unwrap();
    });
}

#[test]
fn fake_runtime_backfills_preexisting_output_files_from_contract() {
    futures::executor::block_on(async {
        let mut workspace = memory_workspace();
        let mut view = workspace.view();
        let output_path = WorkspacePath::new("output/preexisting.txt").unwrap();
        view.write_file(&output_path, b"already there").unwrap();
        let runtime = FakeAgentRuntime::new(Vec::new());

        let metered = runtime
            .run_session(
                &mut view,
                AgentRunRequest::new(
                    AgentInstructions::task("use existing output"),
                    OutputContract::Files {
                        paths: vec![output_path.clone()],
                    },
                ),
                AgentRunContext::new(AgentSessionId::new(), &BudgetSnapshot::default()),
            )
            .await
            .unwrap();

        assert_eq!(metered.value.output_files, vec![output_path]);

        drop(view);
        workspace.cleanup().await.unwrap();
    });
}

#[test]
fn fake_runtime_exposes_identity_capabilities_system_prompt_reads_and_status() {
    futures::executor::block_on(async {
        let mut workspace = memory_workspace();
        let mut view = workspace.view();
        let input_path = WorkspacePath::new("input/context.txt").unwrap();
        view.write_file(&input_path, b"context").unwrap();
        let runtime = FakeAgentRuntime::new(vec![
            FakeAgentAction::ReadFile {
                path: input_path.clone(),
            },
            FakeAgentAction::AssistantMessage("ready".to_owned()),
            FakeAgentAction::Status(leaven_agent::AgentStatus::Failed {
                reason: "contract decided this failed".to_owned(),
            }),
        ])
        .with_id(AgentRuntimeId::from("fake/custom".to_owned()))
        .with_capabilities(AgentRuntimeCapabilities {
            workspace_access: WorkspaceAccessMode::BackendNeutral,
            supports_commands: false,
            supports_raw_provider_events: false,
        });
        let mut instructions = AgentInstructions::task("read context");
        instructions.system = Some("system contract".to_owned());

        assert_eq!(runtime.id().as_str(), "fake/custom");
        assert_eq!(runtime.fingerprint(), Fingerprint::from_bytes([0xFA; 32]));
        assert!(!runtime.capabilities().supports_commands);
        let metered = runtime
            .run_session(
                &mut view,
                AgentRunRequest::new(instructions, OutputContract::FinalMessage),
                AgentRunContext::new(AgentSessionId::new(), &BudgetSnapshot::default()),
            )
            .await
            .unwrap();

        assert!(matches!(
            metered.value.status,
            leaven_agent::AgentStatus::Failed { .. }
        ));
        assert!(metered.value.transcript.events.iter().any(|event| {
            matches!(
                event,
                TranscriptEvent::Message {
                    role: TranscriptRole::System,
                    content,
                } if content == "system contract"
            )
        }));
        assert!(metered.value.transcript.events.iter().any(|event| {
            matches!(
                event,
                TranscriptEvent::Message {
                    role: TranscriptRole::Tool,
                    content,
                } if content.contains("read 7 byte(s)")
            )
        }));

        drop(view);
        workspace.cleanup().await.unwrap();
    });
}

#[test]
fn runtime_trait_default_capabilities_are_backend_neutral() {
    let runtime = DefaultCapabilitiesRuntime;

    assert_eq!(runtime.capabilities(), AgentRuntimeCapabilities::default());
}

#[test]
fn fake_runtime_respects_cancellation_before_actions() {
    futures::executor::block_on(async {
        let mut workspace = memory_workspace();
        let mut view = workspace.view();
        let cancelled = AtomicBool::new(true);
        let output_path = WorkspacePath::new("output/should-not-exist.txt").unwrap();
        let runtime = FakeAgentRuntime::new(vec![FakeAgentAction::WriteFile {
            path: output_path.clone(),
            bytes: b"no".to_vec(),
        }]);
        let budget = BudgetSnapshot::default();

        let metered = runtime
            .run_session(
                &mut view,
                AgentRunRequest::new(
                    AgentInstructions::task("cancel"),
                    OutputContract::WorkspaceDiff { roots: Vec::new() },
                ),
                AgentRunContext::new(AgentSessionId::new(), &budget)
                    .with_cancellation(CancellationRef::from_flag(&cancelled)),
            )
            .await
            .unwrap();

        assert_eq!(metered.value.status, leaven_agent::AgentStatus::Cancelled);
        assert!(view.read_file(&output_path).is_err());
        assert!(
            AgentRunContext::new(AgentSessionId::new(), &budget)
                .budget()
                .spent
                .is_zero()
        );
        cancelled.store(false, Ordering::SeqCst);

        drop(view);
        workspace.cleanup().await.unwrap();
    });
}

#[test]
fn fake_runtime_reports_output_contract_violations_with_sources() {
    futures::executor::block_on(async {
        let missing = WorkspacePath::new("output/missing.json").unwrap();

        for contract in [
            OutputContract::Files {
                paths: vec![missing.clone()],
            },
            OutputContract::JsonFile {
                path: missing.clone(),
                schema: None,
            },
            OutputContract::FinalMessage,
        ] {
            let mut workspace = memory_workspace();
            let mut view = workspace.view();
            let error = FakeAgentRuntime::new(Vec::new())
                .run_session(
                    &mut view,
                    AgentRunRequest::new(AgentInstructions::task("omit output"), contract),
                    AgentRunContext::new(AgentSessionId::new(), &BudgetSnapshot::default()),
                )
                .await
                .unwrap_err();

            match error {
                AgentRuntimeError::WithSource { message, source } => {
                    assert!(message.contains("required"));
                    assert!(source.to_string().contains("missing"));
                }
                AgentRuntimeError::OutputContract(message) => {
                    assert_eq!(message, "final assistant message was required");
                }
                other => panic!("unexpected error: {other:?}"),
            }

            drop(view);
            workspace.cleanup().await.unwrap();
        }
    });
}

#[test]
fn fake_runtime_rejects_invalid_json_output_files() {
    futures::executor::block_on(async {
        let mut workspace = memory_workspace();
        let mut view = workspace.view();
        let output_path = WorkspacePath::new("output/result.json").unwrap();
        let runtime = FakeAgentRuntime::new(vec![FakeAgentAction::WriteFile {
            path: output_path.clone(),
            bytes: b"{not-json".to_vec(),
        }]);

        let error = runtime
            .run_session(
                &mut view,
                AgentRunRequest::new(
                    AgentInstructions::task("write invalid json"),
                    OutputContract::JsonFile {
                        path: output_path.clone(),
                        schema: None,
                    },
                ),
                AgentRunContext::new(AgentSessionId::new(), &BudgetSnapshot::default()),
            )
            .await
            .unwrap_err();

        match error {
            AgentRuntimeError::WithSource { message, source } => {
                assert!(message.contains(output_path.as_str()));
                assert!(!source.to_string().is_empty());
            }
            other => panic!("unexpected error: {other:?}"),
        }

        drop(view);
        workspace.cleanup().await.unwrap();
    });
}

#[test]
fn fake_runtime_records_backend_commands_without_host_paths() {
    futures::executor::block_on(async {
        let mut workspace = memory_workspace();
        let mut view = workspace
            .view()
            .subdir(WorkspacePath::new("case").unwrap())
            .unwrap();
        let runtime = FakeAgentRuntime::new(vec![FakeAgentAction::RunCommand(Command {
            program: "compile".to_owned(),
            args: vec!["--check".to_owned()],
            cwd: Some(WorkspacePath::new("repo").unwrap()),
        })]);

        let metered = runtime
            .run_session(
                &mut view,
                AgentRunRequest::new(
                    AgentInstructions::task("run"),
                    OutputContract::WorkspaceDiff {
                        roots: vec![WorkspacePath::new("case").unwrap()],
                    },
                ),
                AgentRunContext::new(AgentSessionId::new(), &BudgetSnapshot::default()),
            )
            .await
            .unwrap();

        assert_eq!(metered.value.commands.len(), 1);
        assert_eq!(
            metered.value.commands[0].output.stdout,
            b"compiled".as_slice()
        );
        assert_eq!(
            metered.value.commands[0]
                .command
                .cwd
                .as_ref()
                .unwrap()
                .as_str(),
            "repo"
        );

        drop(view);
        workspace.cleanup().await.unwrap();
    });
}

#[test]
fn runtime_requiring_local_mount_fails_early_on_no_mount_backend() {
    futures::executor::block_on(async {
        let mut workspace = memory_workspace();
        let mut view = workspace.view();
        let path = WorkspacePath::new("output/result.txt").unwrap();
        let runtime = FakeAgentRuntime::requiring_local_mount(vec![FakeAgentAction::WriteFile {
            path: path.clone(),
            bytes: b"should not happen".to_vec(),
        }]);

        let error = runtime
            .run_session(
                &mut view,
                AgentRunRequest::new(
                    AgentInstructions::task("write"),
                    OutputContract::Files {
                        paths: vec![path.clone()],
                    },
                ),
                AgentRunContext::new(AgentSessionId::new(), &BudgetSnapshot::default()),
            )
            .await
            .unwrap_err();

        assert!(matches!(
            error,
            AgentRuntimeError::LocalMountRequired { .. }
        ));
        assert!(view.read_file(&path).is_err());

        drop(view);
        workspace.cleanup().await.unwrap();
    });
}

struct DefaultCapabilitiesRuntime;

impl AgentRuntime for DefaultCapabilitiesRuntime {
    fn id(&self) -> AgentRuntimeId {
        AgentRuntimeId::from("default-capabilities")
    }

    fn fingerprint(&self) -> Fingerprint {
        Fingerprint::from_bytes([0xDD; 32])
    }

    async fn run_session(
        &self,
        _workspace: &mut WorkspaceView<'_>,
        _request: AgentRunRequest,
        _ctx: AgentRunContext<'_>,
    ) -> Result<leaven_kernel::Metered<leaven_agent::AgentSession>, AgentRuntimeError> {
        unreachable!("default capability contract does not execute a session")
    }
}

fn memory_workspace() -> Workspace {
    Workspace::new(PathBuf::new(), Box::<MemoryBackend>::default())
}

#[derive(Default)]
struct MemoryBackend {
    files: BTreeMap<WorkspacePath, Vec<u8>>,
}

impl WorkspaceBackend for MemoryBackend {
    fn write_file(&mut self, path: &WorkspacePath, bytes: &[u8]) -> Result<(), WorkspaceError> {
        self.files.insert(path.clone(), bytes.to_vec());
        Ok(())
    }

    fn read_file(&mut self, path: &WorkspacePath) -> Result<Vec<u8>, WorkspaceError> {
        self.files
            .get(path)
            .cloned()
            .ok_or_else(|| WorkspaceError::Io(format!("missing {}", path.as_str())))
    }

    fn run_command(&mut self, command: Command) -> Result<CommandOutput, WorkspaceError> {
        let _ = command;
        Ok(CommandOutput {
            status: ExitStatus { code: Some(0) },
            stdout: b"compiled".to_vec(),
            stderr: Vec::new(),
        })
    }

    fn cleanup(self: Box<Self>) -> BoxFuture<'static, Result<(), WorkspaceError>> {
        async { Ok(()) }.boxed()
    }

    fn local_mount(&self) -> Option<&Path> {
        None
    }
}
