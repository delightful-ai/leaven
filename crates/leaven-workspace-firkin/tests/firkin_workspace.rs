use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures::executor::block_on;
use leaven_workspace::{
    CapturedOutput, Command, CommandLimits, CommandStdin, ExitStatus, WorkspaceConfig,
    WorkspaceError, WorkspaceFactory, WorkspacePath,
};
use leaven_workspace_firkin::{
    FirkinCommandRequest, FirkinCommandResult, FirkinContainerId, FirkinGuestPath, FirkinImageRef,
    FirkinProductPodId, FirkinRuntimeError, FirkinWorkspaceAllocation, FirkinWorkspaceContext,
    FirkinWorkspaceFactory, FirkinWorkspaceRuntime,
};

#[test]
fn factory_allocates_container_in_product_pod_and_attaches_context() {
    block_on(async {
        let runtime = Arc::new(FakeFirkinRuntime::default());
        let factory = factory(runtime.clone());

        let workspace = factory.allocate(WorkspaceConfig::default()).await.unwrap();

        assert!(workspace.local_mount().is_none());
        let context = workspace
            .factory_context::<FirkinWorkspaceContext>()
            .unwrap();
        assert_eq!(context.product_pod_id().as_str(), "pod-run-1");
        assert_eq!(context.container_id().as_str(), "container-1");
        let root = context.workspace_root().as_str();
        assert!(root.starts_with("/workspace/workspaces/"));
        assert_eq!(context.image().as_str(), "ghcr.io/leaven/agent:latest");
        assert_eq!(
            runtime.events(),
            [RuntimeEvent::Allocate {
                pod: "pod-run-1".to_owned(),
                root: root.to_owned(),
                volume_mount_root: "/workspace".to_owned(),
                image: "ghcr.io/leaven/agent:latest".to_owned(),
            }]
        );

        workspace.cleanup().await.unwrap();
        assert_eq!(
            runtime.events().last(),
            Some(&RuntimeEvent::Remove {
                container: "container-1".to_owned(),
            })
        );
    });
}

#[test]
fn backend_routes_file_and_command_operations_to_workspace_root() {
    block_on(async {
        let runtime = Arc::new(FakeFirkinRuntime::default());
        let factory = factory(runtime.clone());
        let mut workspace = factory.allocate(WorkspaceConfig::default()).await.unwrap();
        let context = workspace
            .factory_context::<FirkinWorkspaceContext>()
            .unwrap();
        let root = context.workspace_root().as_str().to_owned();
        let mut view = workspace.view();
        let source = workspace_path("src/main.rs");

        view.write_file(&source, b"fn main() {}\n").unwrap();
        assert_eq!(view.read_file(&source).unwrap(), b"fn main() {}\n");
        assert_eq!(view.list_files(&workspace_path("src")).unwrap(), [source]);

        let mut command = Command::new("sh");
        command.args = vec!["-lc".to_owned(), "printf ok".to_owned()];
        command.cwd = Some(workspace_path("src"));
        command.stdin = CommandStdin::Bytes(b"input".to_vec());
        command.limits = CommandLimits {
            timeout: Some(Duration::from_secs(2)),
            max_stdout_bytes: Some(16),
            max_stderr_bytes: Some(8),
            max_output_file_bytes: None,
        };
        let output = view.run_command(command).unwrap();

        assert_eq!(output.status.code, Some(7));
        assert_eq!(output.stdout.bytes, b"ok\n");
        assert_eq!(output.stderr.bytes, b"warn\n");
        assert!(runtime.events().contains(&RuntimeEvent::Write {
            container: "container-1".to_owned(),
            path: format!("{root}/src/main.rs"),
            bytes: b"fn main() {}\n".to_vec(),
        }));
        assert!(runtime.events().contains(&RuntimeEvent::Read {
            container: "container-1".to_owned(),
            path: format!("{root}/src/main.rs"),
        }));
        assert!(runtime.events().contains(&RuntimeEvent::List {
            container: "container-1".to_owned(),
            path: format!("{root}/src"),
        }));
        assert!(runtime.events().contains(&RuntimeEvent::Run {
            container: "container-1".to_owned(),
            program: "sh".to_owned(),
            args: vec!["-lc".to_owned(), "printf ok".to_owned()],
            cwd: format!("{root}/src"),
            stdin: b"input".to_vec(),
            timeout: Some(Duration::from_secs(2)),
            max_stdout_bytes: Some(16),
            max_stderr_bytes: Some(8),
        }));

        drop(view);
        workspace.cleanup().await.unwrap();
    });
}

#[test]
fn backend_preserves_command_output_byte_limits() {
    block_on(async {
        let runtime = Arc::new(FakeFirkinRuntime::with_command_output(
            b"stdout-too-long".to_vec(),
            b"stderr-too-long".to_vec(),
        ));
        let factory = factory(runtime.clone());
        let mut workspace = factory.allocate(WorkspaceConfig::default()).await.unwrap();
        let context = workspace
            .factory_context::<FirkinWorkspaceContext>()
            .unwrap();
        let root = context.workspace_root().as_str().to_owned();
        let mut view = workspace.view();

        let mut command = Command::new("emit");
        command.limits = CommandLimits {
            timeout: None,
            max_stdout_bytes: Some(6),
            max_stderr_bytes: Some(7),
            max_output_file_bytes: None,
        };
        let output = view.run_command(command).unwrap();

        assert_eq!(output.stdout.bytes, b"stdout");
        assert!(output.stdout.truncated);
        assert_eq!(output.stderr.bytes, b"stderr-");
        assert!(output.stderr.truncated);
        assert!(runtime.events().contains(&RuntimeEvent::Run {
            container: "container-1".to_owned(),
            program: "emit".to_owned(),
            args: Vec::new(),
            cwd: root,
            stdin: Vec::new(),
            timeout: None,
            max_stdout_bytes: Some(6),
            max_stderr_bytes: Some(7),
        }));

        drop(view);
        workspace.cleanup().await.unwrap();
    });
}

#[test]
fn executable_bit_operations_are_explicitly_unsupported() {
    block_on(async {
        let runtime = Arc::new(FakeFirkinRuntime::default());
        let factory = factory(runtime);
        let mut workspace = factory.allocate(WorkspaceConfig::default()).await.unwrap();
        let mut view = workspace.view();

        let set = view.set_executable(&workspace_path("bin/tool"), true);
        assert!(matches!(
            set,
            Err(WorkspaceError::UnsupportedOperation {
                operation: "set_executable"
            })
        ));
        let get = view.is_executable(&workspace_path("bin/tool"));
        assert!(matches!(
            get,
            Err(WorkspaceError::UnsupportedOperation {
                operation: "is_executable"
            })
        ));

        drop(view);
        workspace.cleanup().await.unwrap();
    });
}

#[test]
fn guest_paths_reject_relative_and_parent_traversal_values() {
    assert!(matches!(
        FirkinGuestPath::new("workspace"),
        Err(FirkinRuntimeError::InvalidPlacement {
            field: "guest path",
            ..
        })
    ));
    assert!(matches!(
        FirkinGuestPath::new("/workspace/../secret"),
        Err(FirkinRuntimeError::InvalidPlacement {
            field: "guest path",
            ..
        })
    ));
    assert_eq!(
        FirkinGuestPath::new("//workspace//run//").unwrap().as_str(),
        "/workspace/run"
    );
}

fn factory(runtime: Arc<FakeFirkinRuntime>) -> FirkinWorkspaceFactory<FakeFirkinRuntime> {
    FirkinWorkspaceFactory::new(
        runtime,
        FirkinProductPodId::new("pod-run-1").unwrap(),
        FirkinGuestPath::new("/workspace").unwrap(),
        FirkinImageRef::new("ghcr.io/leaven/agent:latest").unwrap(),
    )
}

fn workspace_path(path: &str) -> WorkspacePath {
    WorkspacePath::new(path).unwrap()
}

#[derive(Default)]
struct FakeFirkinRuntime {
    state: Mutex<FakeState>,
}

impl FakeFirkinRuntime {
    fn with_command_output(stdout: Vec<u8>, stderr: Vec<u8>) -> Self {
        Self {
            state: Mutex::new(FakeState {
                command_stdout: stdout,
                command_stderr: stderr,
                ..FakeState::default()
            }),
        }
    }

    fn events(&self) -> Vec<RuntimeEvent> {
        self.state.lock().unwrap().events.clone()
    }
}

struct FakeState {
    next_container: usize,
    files: Vec<(String, Vec<u8>)>,
    events: Vec<RuntimeEvent>,
    command_stdout: Vec<u8>,
    command_stderr: Vec<u8>,
}

impl Default for FakeState {
    fn default() -> Self {
        Self {
            next_container: 0,
            files: Vec::new(),
            events: Vec::new(),
            command_stdout: b"ok\n".to_vec(),
            command_stderr: b"warn\n".to_vec(),
        }
    }
}

impl FirkinWorkspaceRuntime for FakeFirkinRuntime {
    fn allocate_container(
        &self,
        request: FirkinWorkspaceAllocation,
    ) -> Result<FirkinContainerId, FirkinRuntimeError> {
        let mut state = self.state.lock().unwrap();
        state.next_container += 1;
        state.events.push(RuntimeEvent::Allocate {
            pod: request.product_pod_id().as_str().to_owned(),
            root: request.workspace_root().as_str().to_owned(),
            volume_mount_root: request.volume_mount_root().as_str().to_owned(),
            image: request.image().as_str().to_owned(),
        });
        FirkinContainerId::new(format!("container-{}", state.next_container))
    }

    fn write_file(
        &self,
        container: &FirkinContainerId,
        path: &FirkinGuestPath,
        bytes: &[u8],
    ) -> Result<(), FirkinRuntimeError> {
        let mut state = self.state.lock().unwrap();
        state.files.push((path.as_str().to_owned(), bytes.to_vec()));
        state.events.push(RuntimeEvent::Write {
            container: container.as_str().to_owned(),
            path: path.as_str().to_owned(),
            bytes: bytes.to_vec(),
        });
        Ok(())
    }

    fn read_file(
        &self,
        container: &FirkinContainerId,
        path: &FirkinGuestPath,
    ) -> Result<Vec<u8>, FirkinRuntimeError> {
        let mut state = self.state.lock().unwrap();
        state.events.push(RuntimeEvent::Read {
            container: container.as_str().to_owned(),
            path: path.as_str().to_owned(),
        });
        state
            .files
            .iter()
            .rev()
            .find(|(file, _)| file == path.as_str())
            .map(|(_, bytes)| bytes.clone())
            .ok_or_else(|| FirkinRuntimeError::Runtime {
                operation: "fake read",
                reason: format!("{} missing", path.as_str()),
            })
    }

    fn list_files(
        &self,
        container: &FirkinContainerId,
        root: &FirkinGuestPath,
    ) -> Result<Vec<WorkspacePath>, FirkinRuntimeError> {
        self.state.lock().unwrap().events.push(RuntimeEvent::List {
            container: container.as_str().to_owned(),
            path: root.as_str().to_owned(),
        });
        Ok(vec![workspace_path("src/main.rs")])
    }

    fn run_command(
        &self,
        container: &FirkinContainerId,
        request: FirkinCommandRequest,
    ) -> Result<FirkinCommandResult, FirkinRuntimeError> {
        self.state.lock().unwrap().events.push(RuntimeEvent::Run {
            container: container.as_str().to_owned(),
            program: request.program,
            args: request.args,
            cwd: request.cwd.as_str().to_owned(),
            stdin: request.stdin,
            timeout: request.timeout,
            max_stdout_bytes: request.max_stdout_bytes,
            max_stderr_bytes: request.max_stderr_bytes,
        });
        let state = self.state.lock().unwrap();
        Ok(FirkinCommandResult {
            status: ExitStatus { code: Some(7) },
            stdout: CapturedOutput {
                bytes: state.command_stdout.clone(),
                truncated: false,
            },
            stderr: CapturedOutput {
                bytes: state.command_stderr.clone(),
                truncated: false,
            },
            duration: Duration::from_millis(9),
        })
    }

    fn remove_container(&self, container: FirkinContainerId) -> Result<(), FirkinRuntimeError> {
        self.state
            .lock()
            .unwrap()
            .events
            .push(RuntimeEvent::Remove {
                container: container.as_str().to_owned(),
            });
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum RuntimeEvent {
    Allocate {
        pod: String,
        root: String,
        volume_mount_root: String,
        image: String,
    },
    Write {
        container: String,
        path: String,
        bytes: Vec<u8>,
    },
    Read {
        container: String,
        path: String,
    },
    List {
        container: String,
        path: String,
    },
    Run {
        container: String,
        program: String,
        args: Vec<String>,
        cwd: String,
        stdin: Vec<u8>,
        timeout: Option<Duration>,
        max_stdout_bytes: Option<u64>,
        max_stderr_bytes: Option<u64>,
    },
    Remove {
        container: String,
    },
}
