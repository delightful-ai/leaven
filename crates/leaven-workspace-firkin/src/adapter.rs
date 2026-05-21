use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use firkin_e2b_contract::{BackendError, RuntimeAdapter};
use firkin_e2b_wire::{PodContainerCreateRequest, PodContainerOutput, PodVolumeMountRequest};
use leaven_workspace::{CapturedOutput, ExitStatus, WorkspacePath};

use crate::{
    FirkinCommandRequest, FirkinCommandResult, FirkinContainerId, FirkinGuestPath, FirkinImageRef,
    FirkinProductPodId, FirkinRuntimeError, FirkinWorkspaceAllocation, FirkinWorkspaceRuntime,
};

#[derive(Clone)]
pub struct FirkinRuntimeAdapterRuntime<A> {
    adapter: A,
    workspace_volume: String,
    sequence: Arc<AtomicU64>,
    runtime: Arc<tokio::runtime::Runtime>,
    workspaces: Arc<Mutex<HashMap<String, ProductPodWorkspace>>>,
}

impl<A> FirkinRuntimeAdapterRuntime<A>
where
    A: RuntimeAdapter,
{
    pub fn new(
        adapter: A,
        workspace_volume: impl Into<String>,
    ) -> Result<Self, FirkinRuntimeError> {
        let workspace_volume = workspace_volume.into();
        if workspace_volume.is_empty() {
            return Err(FirkinRuntimeError::InvalidPlacement {
                field: "workspace volume",
                value: workspace_volume,
                reason: "must not be empty",
            });
        }
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|source| FirkinRuntimeError::Runtime {
                operation: "create Firkin adapter runtime",
                reason: source.to_string(),
            })?;
        Ok(Self {
            adapter,
            workspace_volume,
            sequence: Arc::new(AtomicU64::new(0)),
            runtime: Arc::new(runtime),
            workspaces: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    fn next_name(&self, prefix: &str) -> String {
        let id = self.sequence.fetch_add(1, Ordering::Relaxed) + 1;
        format!("{prefix}-{id}")
    }

    fn block_on<T>(
        &self,
        future: impl std::future::Future<Output = Result<T, BackendError>>,
    ) -> Result<T, FirkinRuntimeError> {
        self.runtime
            .block_on(future)
            .map_err(|source| FirkinRuntimeError::Runtime {
                operation: "call Firkin product pod adapter",
                reason: source.to_string(),
            })
    }

    fn workspace_for(
        &self,
        container: &FirkinContainerId,
    ) -> Result<ProductPodWorkspace, FirkinRuntimeError> {
        self.workspaces
            .lock()
            .map_err(lock_error)?
            .get(container.as_str())
            .cloned()
            .ok_or_else(|| FirkinRuntimeError::Runtime {
                operation: "resolve Firkin workspace container",
                reason: format!("container `{container}` was not allocated by this runtime"),
            })
    }

    fn mount_for(&self, workspace: &ProductPodWorkspace) -> PodVolumeMountRequest {
        PodVolumeMountRequest {
            name: self.workspace_volume.clone(),
            path: workspace.root.as_str().to_owned(),
            read_only: false,
        }
    }

    fn container_request(
        &self,
        name: String,
        workspace: &ProductPodWorkspace,
        command: Vec<String>,
        env_vars: BTreeMap<String, String>,
        capture_output: bool,
    ) -> PodContainerCreateRequest {
        PodContainerCreateRequest {
            name,
            template_id: workspace.image.as_str().to_owned(),
            command,
            env_vars,
            empty_dir_mounts: vec![self.mount_for(workspace)],
            capture_output,
        }
    }

    fn run_helper(
        &self,
        workspace: &ProductPodWorkspace,
        command: Vec<String>,
        env_vars: BTreeMap<String, String>,
    ) -> Result<PodContainerOutput, FirkinRuntimeError> {
        let name = self.next_name("leaven-helper");
        let request = self.container_request(name.clone(), workspace, command, env_vars, true);
        self.block_on(
            self.adapter
                .add_pod_container(workspace.pod.as_str(), request),
        )?;
        let output = self.block_on(
            self.adapter
                .wait_pod_container(workspace.pod.as_str(), &name),
        );
        let remove = self.block_on(
            self.adapter
                .remove_pod_container(workspace.pod.as_str(), &name),
        );
        match (output, remove) {
            (Ok(output), Ok(())) => Ok(output),
            (Err(error), Ok(())) | (Ok(_), Err(error)) => Err(error),
            (Err(error), Err(_cleanup)) => Err(error),
        }
    }

    fn run_checked_helper(
        &self,
        workspace: &ProductPodWorkspace,
        command: Vec<String>,
        env_vars: BTreeMap<String, String>,
        operation: &'static str,
    ) -> Result<PodContainerOutput, FirkinRuntimeError> {
        let output = self.run_helper(workspace, command, env_vars)?;
        if output.exit_code == 0 {
            return Ok(output);
        }
        Err(FirkinRuntimeError::Runtime {
            operation,
            reason: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }
}

impl<A> FirkinWorkspaceRuntime for FirkinRuntimeAdapterRuntime<A>
where
    A: RuntimeAdapter,
{
    fn allocate_container(
        &self,
        request: FirkinWorkspaceAllocation,
    ) -> Result<FirkinContainerId, FirkinRuntimeError> {
        let name = self.next_name("leaven-ws");
        let workspace = ProductPodWorkspace {
            pod: request.product_pod_id().clone(),
            root: request.workspace_root().clone(),
            image: request.image().clone(),
        };
        let container = self.container_request(
            name.clone(),
            &workspace,
            vec![
                "sh".to_owned(),
                "-lc".to_owned(),
                "sleep infinity".to_owned(),
            ],
            BTreeMap::new(),
            false,
        );
        self.block_on(
            self.adapter
                .add_pod_container(workspace.pod.as_str(), container),
        )?;
        self.workspaces
            .lock()
            .map_err(lock_error)?
            .insert(name.clone(), workspace);
        FirkinContainerId::new(name)
    }

    fn write_file(
        &self,
        container: &FirkinContainerId,
        path: &FirkinGuestPath,
        bytes: &[u8],
    ) -> Result<(), FirkinRuntimeError> {
        let workspace = self.workspace_for(container)?;
        let mut env = BTreeMap::new();
        env.insert("LEAVEN_PATH".to_owned(), path.as_str().to_owned());
        env.insert("LEAVEN_BYTES_OCTAL".to_owned(), shell_octal_bytes(bytes));
        self.run_checked_helper(
            &workspace,
            vec![
                "sh".to_owned(),
                "-lc".to_owned(),
                "mkdir -p -- \"$(dirname -- \"$LEAVEN_PATH\")\" && printf '%b' \"$LEAVEN_BYTES_OCTAL\" > \"$LEAVEN_PATH\""
                    .to_owned(),
            ],
            env,
            "write Firkin workspace file",
        )?;
        Ok(())
    }

    fn read_file(
        &self,
        container: &FirkinContainerId,
        path: &FirkinGuestPath,
    ) -> Result<Vec<u8>, FirkinRuntimeError> {
        let workspace = self.workspace_for(container)?;
        let output = self.run_checked_helper(
            &workspace,
            vec![
                "sh".to_owned(),
                "-lc".to_owned(),
                "cat -- \"$LEAVEN_PATH\"".to_owned(),
            ],
            BTreeMap::from([("LEAVEN_PATH".to_owned(), path.as_str().to_owned())]),
            "read Firkin workspace file",
        )?;
        Ok(output.stdout)
    }

    fn list_files(
        &self,
        container: &FirkinContainerId,
        root: &FirkinGuestPath,
    ) -> Result<Vec<WorkspacePath>, FirkinRuntimeError> {
        let workspace = self.workspace_for(container)?;
        let output = self.run_checked_helper(
            &workspace,
            vec![
                "sh".to_owned(),
                "-lc".to_owned(),
                "find \"$LEAVEN_PATH\" -type f -print".to_owned(),
            ],
            BTreeMap::from([("LEAVEN_PATH".to_owned(), root.as_str().to_owned())]),
            "list Firkin workspace files",
        )?;
        parse_find_output(&workspace.root, &output.stdout)
    }

    fn run_command(
        &self,
        container: &FirkinContainerId,
        request: FirkinCommandRequest,
    ) -> Result<FirkinCommandResult, FirkinRuntimeError> {
        if !request.stdin.is_empty() {
            return Err(FirkinRuntimeError::Runtime {
                operation: "run Firkin workspace command",
                reason: "product pod container requests do not accept stdin".to_owned(),
            });
        }
        if request.user.is_some() {
            return Err(FirkinRuntimeError::Runtime {
                operation: "run Firkin workspace command",
                reason: "product pod container requests do not accept user overrides".to_owned(),
            });
        }
        let workspace = self.workspace_for(container)?;
        let mut env = request.env;
        env.insert("LEAVEN_CWD".to_owned(), request.cwd.as_str().to_owned());
        if let Some(timeout) = request.timeout {
            env.insert(
                "LEAVEN_TIMEOUT_SECONDS".to_owned(),
                timeout.as_secs().to_string(),
            );
        }
        let mut command = vec![
            "sh".to_owned(),
            "-lc".to_owned(),
            "cd \"$LEAVEN_CWD\" && exec \"$@\"".to_owned(),
            "leaven-run".to_owned(),
            request.program,
        ];
        command.extend(request.args);
        let started = std::time::Instant::now();
        let output = self.run_helper(&workspace, command, env)?;
        Ok(FirkinCommandResult {
            status: ExitStatus {
                code: Some(output.exit_code),
            },
            stdout: CapturedOutput::new(output.stdout, request.max_stdout_bytes),
            stderr: CapturedOutput::new(output.stderr, request.max_stderr_bytes),
            duration: started.elapsed().max(Duration::from_nanos(1)),
        })
    }

    fn remove_container(&self, container: FirkinContainerId) -> Result<(), FirkinRuntimeError> {
        let workspace = self
            .workspaces
            .lock()
            .map_err(lock_error)?
            .remove(container.as_str())
            .ok_or_else(|| FirkinRuntimeError::Runtime {
                operation: "remove Firkin workspace container",
                reason: format!("container `{container}` was not allocated by this runtime"),
            })?;
        self.block_on(
            self.adapter
                .remove_pod_container(workspace.pod.as_str(), container.as_str()),
        )
    }
}

#[derive(Clone)]
struct ProductPodWorkspace {
    pod: FirkinProductPodId,
    root: FirkinGuestPath,
    image: FirkinImageRef,
}

fn shell_octal_bytes(bytes: &[u8]) -> String {
    let mut encoded = String::new();
    for byte in bytes {
        write!(&mut encoded, "\\{byte:03o}").expect("writing to String cannot fail");
    }
    encoded
}

fn parse_find_output(
    workspace_root: &FirkinGuestPath,
    output: &[u8],
) -> Result<Vec<WorkspacePath>, FirkinRuntimeError> {
    let text = String::from_utf8_lossy(output);
    let prefix = format!("{}/", workspace_root.as_str().trim_end_matches('/'));
    let mut files = Vec::new();
    for line in text.lines().filter(|line| !line.is_empty()) {
        let Some(relative) = line.strip_prefix(&prefix) else {
            return Err(FirkinRuntimeError::Runtime {
                operation: "list Firkin workspace files",
                reason: format!("runtime returned path `{line}` outside workspace root"),
            });
        };
        files.push(WorkspacePath::new(relative)?);
    }
    files.sort();
    Ok(files)
}

fn lock_error<T>(_error: std::sync::PoisonError<T>) -> FirkinRuntimeError {
    FirkinRuntimeError::Runtime {
        operation: "lock Firkin adapter state",
        reason: "lock poisoned".to_owned(),
    }
}
