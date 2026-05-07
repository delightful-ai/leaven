use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use futures::future::{BoxFuture, FutureExt};
use leaven_agent::{
    AgentInstructions, AgentRunContext, AgentRunRequest, AgentRuntime, AgentRuntimeError,
    FakeAgentAction, FakeAgentRuntime, OutputContract, TranscriptEvent, TranscriptRole,
};
use leaven_kernel::{AgentSessionId, BudgetSnapshot, Cost};
use leaven_workspace::{
    Command, CommandOutput, ExitStatus, Workspace, WorkspaceBackend, WorkspaceError, WorkspacePath,
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
