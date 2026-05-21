#![cfg(feature = "firkin-facade")]

use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use firkin_e2b_contract::{
    BackendError, FollowupSnapshot, PausedSandbox, PortProxyStream, PortTarget, PreparedTemplate,
    RuntimeAdapter, RuntimeCapabilitySet, RuntimePod, RuntimeSandbox, SnapshotRef, StartPodRequest,
    StartSandboxRequest,
};
use firkin_e2b_wire::{
    PodContainerCreateRequest, PodContainerInfo, PodContainerOutput, PodVolumeMountRequest,
    SandboxLogs, SandboxMetric, TemplateBuildRequest,
};
use firkin_types::SandboxNetworkPolicy;
use leaven_workspace::{CapturedOutput, CommandUser, ExitStatus, WorkspacePath};
use leaven_workspace_firkin::{
    FirkinCommandRequest, FirkinGuestPath, FirkinImageRef, FirkinProductPodId,
    FirkinRuntimeAdapterRuntime, FirkinRuntimeError, FirkinWorkspaceAllocation,
    FirkinWorkspaceRuntime,
};

#[test]
fn runtime_adapter_uses_product_pod_containers_for_workspace_commands() {
    let adapter = RecordingRuntimeAdapter::default();
    let runtime = FirkinRuntimeAdapterRuntime::new(adapter.clone(), "workspace-volume").unwrap();
    let pod = FirkinProductPodId::new("run-pod").unwrap();
    let root = FirkinGuestPath::new("/workspace").unwrap();
    let image = FirkinImageRef::new("agent-template").unwrap();

    let container = runtime
        .allocate_container(FirkinWorkspaceAllocation::new(pod, root.clone(), image))
        .unwrap();
    let output = runtime
        .run_command(
            &container,
            FirkinCommandRequest {
                program: "sh".to_owned(),
                args: vec!["-lc".to_owned(), "printf ok".to_owned()],
                cwd: root
                    .join_workspace_path(&leaven_workspace::WorkspacePath::new("repo").unwrap()),
                env: BTreeMap::from([("A".to_owned(), "B".to_owned())]),
                stdin: Vec::new(),
                timeout: Some(Duration::from_secs(5)),
                max_stdout_bytes: Some(8),
                max_stderr_bytes: Some(8),
                user: None,
            },
        )
        .unwrap();
    runtime.remove_container(container).unwrap();

    assert_eq!(
        output.stdout,
        CapturedOutput {
            bytes: b"ok\n".to_vec(),
            truncated: false,
        }
    );
    assert_eq!(output.status, ExitStatus { code: Some(0) });

    let events = adapter.events();
    let [
        AdapterEvent::Add {
            pod_id: anchor_pod,
            container: anchor,
        },
        AdapterEvent::Add {
            pod_id: command_pod,
            container: command,
        },
        AdapterEvent::Wait {
            pod_id: wait_pod,
            container_name: wait_container,
        },
        AdapterEvent::Remove {
            pod_id: command_remove_pod,
            container_name: command_remove,
        },
        AdapterEvent::Remove {
            pod_id: anchor_remove_pod,
            container_name: anchor_remove,
        },
    ] = events.as_slice()
    else {
        panic!("unexpected adapter events: {events:#?}");
    };

    assert_eq!(anchor_pod, "run-pod");
    assert_eq!(anchor.template_id, "agent-template");
    assert_eq!(
        anchor.empty_dir_mounts,
        [PodVolumeMountRequest {
            name: "workspace-volume".to_owned(),
            path: "/workspace".to_owned(),
            read_only: false,
        }]
    );
    assert_eq!(anchor.command, ["sh", "-lc", "sleep infinity"]);
    assert!(!anchor.capture_output);

    assert_eq!(command_pod, "run-pod");
    assert_eq!(command.template_id, "agent-template");
    assert_eq!(command.empty_dir_mounts, anchor.empty_dir_mounts);
    assert_eq!(
        command.command,
        [
            "sh",
            "-lc",
            "cd \"$LEAVEN_CWD\" && exec \"$@\"",
            "leaven-run",
            "sh",
            "-lc",
            "printf ok"
        ]
    );
    assert_eq!(command.env_vars["A"], "B");
    assert_eq!(command.env_vars["LEAVEN_CWD"], "/workspace/repo");
    assert!(command.capture_output);
    assert_eq!(wait_pod, "run-pod");
    assert_eq!(wait_container, &command.name);
    assert_eq!(command_remove_pod, "run-pod");
    assert_eq!(command_remove, &command.name);
    assert_eq!(anchor_remove_pod, "run-pod");
    assert_eq!(anchor_remove, &anchor.name);
}

#[test]
fn runtime_adapter_uses_product_pod_helpers_for_workspace_file_operations() {
    let adapter = RecordingRuntimeAdapter::with_outputs(vec![
        PodContainerOutput::new(Vec::new(), Vec::new(), 0),
        PodContainerOutput::new(b"file bytes".to_vec(), Vec::new(), 0),
        PodContainerOutput::new(
            b"/workspace/repo/main.rs\n/workspace/repo/lib.rs\n".to_vec(),
            Vec::new(),
            0,
        ),
    ]);
    let runtime = FirkinRuntimeAdapterRuntime::new(adapter.clone(), "workspace-volume").unwrap();
    let pod = FirkinProductPodId::new("run-pod").unwrap();
    let root = FirkinGuestPath::new("/workspace").unwrap();
    let image = FirkinImageRef::new("agent-template").unwrap();
    let file = root.join_workspace_path(&WorkspacePath::new("repo/main.rs").unwrap());
    let repo = root.join_workspace_path(&WorkspacePath::new("repo").unwrap());

    let container = runtime
        .allocate_container(FirkinWorkspaceAllocation::new(pod, root, image))
        .unwrap();
    runtime
        .write_file(&container, &file, b"fn main() {}\n")
        .unwrap();
    assert_eq!(runtime.read_file(&container, &file).unwrap(), b"file bytes");
    assert_eq!(
        runtime.list_files(&container, &repo).unwrap(),
        [
            WorkspacePath::new("repo/lib.rs").unwrap(),
            WorkspacePath::new("repo/main.rs").unwrap()
        ]
    );
    runtime.remove_container(container).unwrap();

    let events = adapter.events();
    let helper_adds: Vec<_> = events
        .iter()
        .filter_map(|event| match event {
            AdapterEvent::Add { container, .. } if container.capture_output => Some(container),
            _ => None,
        })
        .collect();
    assert_eq!(helper_adds.len(), 3, "events: {events:#?}");

    let write = helper_adds[0];
    assert_eq!(
        write.command,
        [
            "sh",
            "-lc",
            "mkdir -p -- \"$(dirname -- \"$LEAVEN_PATH\")\" && printf '%b' \"$LEAVEN_BYTES_OCTAL\" > \"$LEAVEN_PATH\""
        ]
    );
    assert_eq!(write.env_vars["LEAVEN_PATH"], "/workspace/repo/main.rs");
    assert_eq!(
        write.env_vars["LEAVEN_BYTES_OCTAL"],
        "\\146\\156\\040\\155\\141\\151\\156\\050\\051\\040\\173\\175\\012"
    );

    let read = helper_adds[1];
    assert_eq!(read.command, ["sh", "-lc", "cat -- \"$LEAVEN_PATH\""]);
    assert_eq!(read.env_vars["LEAVEN_PATH"], "/workspace/repo/main.rs");

    let list = helper_adds[2];
    assert_eq!(
        list.command,
        ["sh", "-lc", "find \"$LEAVEN_PATH\" -type f -print"]
    );
    assert_eq!(list.env_vars["LEAVEN_PATH"], "/workspace/repo");
}

#[test]
fn runtime_adapter_rejects_command_options_without_product_pod_support() {
    let adapter = RecordingRuntimeAdapter::default();
    let runtime = FirkinRuntimeAdapterRuntime::new(adapter, "workspace-volume").unwrap();
    let pod = FirkinProductPodId::new("run-pod").unwrap();
    let root = FirkinGuestPath::new("/workspace").unwrap();
    let image = FirkinImageRef::new("agent-template").unwrap();

    let container = runtime
        .allocate_container(FirkinWorkspaceAllocation::new(pod, root.clone(), image))
        .unwrap();

    let stdin_error = runtime
        .run_command(
            &container,
            FirkinCommandRequest {
                program: "cat".to_owned(),
                args: Vec::new(),
                cwd: root.clone(),
                env: BTreeMap::new(),
                stdin: b"input".to_vec(),
                timeout: None,
                max_stdout_bytes: None,
                max_stderr_bytes: None,
                user: None,
            },
        )
        .unwrap_err();
    assert!(matches!(
        stdin_error,
        FirkinRuntimeError::Runtime {
            operation: "run Firkin workspace command",
            ..
        }
    ));
    assert!(stdin_error.to_string().contains("do not accept stdin"));

    let user_error = runtime
        .run_command(
            &container,
            FirkinCommandRequest {
                program: "id".to_owned(),
                args: Vec::new(),
                cwd: root,
                env: BTreeMap::new(),
                stdin: Vec::new(),
                timeout: None,
                max_stdout_bytes: None,
                max_stderr_bytes: None,
                user: Some(CommandUser::Name("agent".to_owned())),
            },
        )
        .unwrap_err();
    assert!(matches!(
        user_error,
        FirkinRuntimeError::Runtime {
            operation: "run Firkin workspace command",
            ..
        }
    ));
    assert!(user_error.to_string().contains("user overrides"));

    runtime.remove_container(container).unwrap();
}

#[derive(Clone, Default)]
struct RecordingRuntimeAdapter {
    state: Arc<Mutex<RecordingState>>,
}

impl RecordingRuntimeAdapter {
    fn with_outputs(outputs: Vec<PodContainerOutput>) -> Self {
        Self {
            state: Arc::new(Mutex::new(RecordingState {
                wait_outputs: outputs.into(),
                ..RecordingState::default()
            })),
        }
    }

    fn events(&self) -> Vec<AdapterEvent> {
        self.state.lock().unwrap().events.clone()
    }
}

#[derive(Default)]
struct RecordingState {
    events: Vec<AdapterEvent>,
    wait_outputs: VecDeque<PodContainerOutput>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum AdapterEvent {
    Add {
        pod_id: String,
        container: PodContainerCreateRequest,
    },
    Wait {
        pod_id: String,
        container_name: String,
    },
    Remove {
        pod_id: String,
        container_name: String,
    },
}

#[async_trait::async_trait]
impl RuntimeAdapter for RecordingRuntimeAdapter {
    async fn preflight(&self) -> Result<RuntimeCapabilitySet, BackendError> {
        Err(BackendError::Runtime("unused".to_owned()))
    }

    async fn prepare_template(
        &self,
        _request: TemplateBuildRequest,
    ) -> Result<PreparedTemplate, BackendError> {
        Err(BackendError::Runtime("unused".to_owned()))
    }

    async fn start(&self, _request: StartSandboxRequest) -> Result<RuntimeSandbox, BackendError> {
        Err(BackendError::Runtime("unused".to_owned()))
    }

    async fn start_followup(
        &self,
        _request: StartSandboxRequest,
        _snapshot: FollowupSnapshot,
    ) -> Result<RuntimeSandbox, BackendError> {
        Err(BackendError::Runtime("unused".to_owned()))
    }

    async fn add_pod_container(
        &self,
        pod_id: &str,
        container: PodContainerCreateRequest,
    ) -> Result<PodContainerInfo, BackendError> {
        let mut state = self.state.lock().unwrap();
        state.events.push(AdapterEvent::Add {
            pod_id: pod_id.to_owned(),
            container: container.clone(),
        });
        if container.capture_output && state.wait_outputs.is_empty() {
            state
                .wait_outputs
                .push_back(PodContainerOutput::new(b"ok\n".to_vec(), Vec::new(), 0));
        }
        Ok(PodContainerInfo::running(&container))
    }

    async fn wait_pod_container(
        &self,
        pod_id: &str,
        container_name: &str,
    ) -> Result<PodContainerOutput, BackendError> {
        let mut state = self.state.lock().unwrap();
        state.events.push(AdapterEvent::Wait {
            pod_id: pod_id.to_owned(),
            container_name: container_name.to_owned(),
        });
        state
            .wait_outputs
            .pop_front()
            .ok_or_else(|| BackendError::Runtime("missing wait output".to_owned()))
    }

    async fn remove_pod_container(
        &self,
        pod_id: &str,
        container_name: &str,
    ) -> Result<(), BackendError> {
        self.state
            .lock()
            .unwrap()
            .events
            .push(AdapterEvent::Remove {
                pod_id: pod_id.to_owned(),
                container_name: container_name.to_owned(),
            });
        Ok(())
    }

    async fn stop(&self, _sandbox_id: &str) -> Result<(), BackendError> {
        Err(BackendError::Runtime("unused".to_owned()))
    }

    async fn pause(&self, _sandbox_id: &str) -> Result<PausedSandbox, BackendError> {
        Err(BackendError::Runtime("unused".to_owned()))
    }

    async fn resume(&self, _paused: PausedSandbox) -> Result<RuntimeSandbox, BackendError> {
        Err(BackendError::Runtime("unused".to_owned()))
    }

    async fn snapshot(
        &self,
        _sandbox_id: &str,
        _name: Option<String>,
    ) -> Result<SnapshotRef, BackendError> {
        Err(BackendError::Runtime("unused".to_owned()))
    }

    async fn metrics(&self, _sandbox_id: &str) -> Result<Vec<SandboxMetric>, BackendError> {
        Err(BackendError::Runtime("unused".to_owned()))
    }

    async fn logs(&self, _sandbox_id: &str) -> Result<SandboxLogs, BackendError> {
        Err(BackendError::Runtime("unused".to_owned()))
    }

    async fn apply_network(
        &self,
        _sandbox_id: &str,
        _policy: SandboxNetworkPolicy,
    ) -> Result<(), BackendError> {
        Err(BackendError::Runtime("unused".to_owned()))
    }

    async fn port_target(&self, _sandbox_id: &str, _port: u16) -> Result<PortTarget, BackendError> {
        Err(BackendError::Runtime("unused".to_owned()))
    }

    async fn connect_port_target(
        &self,
        _sandbox_id: &str,
        _target: PortTarget,
    ) -> Result<PortProxyStream, BackendError> {
        Err(BackendError::Runtime("unused".to_owned()))
    }

    async fn start_pod(&self, _request: StartPodRequest) -> Result<RuntimePod, BackendError> {
        Err(BackendError::Runtime("unused".to_owned()))
    }
}
