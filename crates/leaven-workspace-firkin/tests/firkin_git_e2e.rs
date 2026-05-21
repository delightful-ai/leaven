use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command as ProcessCommand, Stdio};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use futures::executor::block_on;
use leaven_agentic_git::{GitProgramMaterializer, GitProgramReadback, GitProgramStores};
use leaven_artifact_git::{
    GitArtifactIdentityMode, GitObjectId, GitPath, GitProgramArtifact, GitProgramChange,
    GitProgramLayout, GitRepoArtifact, GitRevision, RepoKey, RepoRef,
};
use leaven_workspace::{
    CapturedOutput, ExitStatus, WorkspaceConfig, WorkspaceFactory, WorkspacePath,
};
use leaven_workspace_firkin::{
    FirkinCommandRequest, FirkinCommandResult, FirkinContainerId, FirkinGuestPath, FirkinImageRef,
    FirkinProductPodId, FirkinRuntimeError, FirkinWorkspaceAllocation, FirkinWorkspaceContext,
    FirkinWorkspaceFactory, FirkinWorkspaceRuntime,
};

#[test]
fn firkin_product_pod_materializes_and_reads_back_isolated_git_workspaces() {
    block_on(async {
        let fixture = GitFixture::new();
        let runtime = Arc::new(HostProductPodRuntime::new("/workspace"));
        let factory = FirkinWorkspaceFactory::new(
            runtime.clone(),
            FirkinProductPodId::new("pod-run-1").unwrap(),
            FirkinGuestPath::new("/workspace").unwrap(),
            FirkinImageRef::new("ghcr.io/leaven/agent:latest").unwrap(),
        );

        let mut workspace_a = factory.allocate(WorkspaceConfig::default()).await.unwrap();
        let mut workspace_b = factory.allocate(WorkspaceConfig::default()).await.unwrap();
        assert!(workspace_a.local_mount().is_none());
        assert!(workspace_b.local_mount().is_none());
        let context_a = workspace_a
            .factory_context::<FirkinWorkspaceContext>()
            .unwrap();
        let context_b = workspace_b
            .factory_context::<FirkinWorkspaceContext>()
            .unwrap();
        assert_eq!(context_a.product_pod_id(), context_b.product_pod_id());
        assert_ne!(context_a.container_id(), context_b.container_id());
        assert_ne!(context_a.workspace_root(), context_b.workspace_root());

        let artifact = fixture.artifact();
        let materializer = GitProgramMaterializer::new(fixture.stores());
        let mut view_a = workspace_a.view();
        let mut view_b = workspace_b.view();
        materializer
            .materialize_program(&artifact, &mut view_a)
            .unwrap();
        materializer
            .materialize_program(&artifact, &mut view_b)
            .unwrap();
        assert_eq!(
            view_a
                .read_file(&workspace_path("repos/program/program.txt"))
                .unwrap(),
            b"program base\n"
        );
        assert_eq!(
            view_b
                .read_file(&workspace_path("repos/program/program.txt"))
                .unwrap(),
            b"program base\n"
        );

        view_a
            .write_file(
                &workspace_path("repos/program/program.txt"),
                b"program child from firkin workspace\n",
            )
            .unwrap();
        let change = GitProgramReadback::new(fixture.stores())
            .read_back_change(&artifact, &mut view_a)
            .unwrap()
            .expect("dirty Firkin workspace should produce a Git change");

        let GitProgramChange::AdvanceRepo { child, .. } = change else {
            panic!("single repo Firkin readback should return AdvanceRepo");
        };
        let GitRevision::Commit(child) = child else {
            panic!("Firkin readback should import a commit");
        };
        assert_eq!(
            git_output(&fixture.store, ["show", &format!("{child}:program.txt")],),
            "program child from firkin workspace\n"
        );
        assert_eq!(
            view_b
                .read_file(&workspace_path("repos/program/program.txt"))
                .unwrap(),
            b"program base\n"
        );

        drop(view_a);
        drop(view_b);
        workspace_a.cleanup().await.unwrap();
        workspace_b.cleanup().await.unwrap();
        assert_eq!(runtime.removed_containers().len(), 2);
    });
}

struct GitFixture {
    store: PathBuf,
    parent: GitRevision,
    _temp: tempfile::TempDir,
}

impl GitFixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source");
        let store = temp.path().join("program.git");
        create_repo(&source);
        run_git_at(temp.path(), ["clone", "--bare", "source", "program.git"]);
        let parent = GitRevision::Commit(git_object(&source, "main"));
        Self {
            store,
            parent,
            _temp: temp,
        }
    }

    fn stores(&self) -> GitProgramStores {
        GitProgramStores::new(BTreeMap::from([(repo_key("program"), self.store.clone())])).unwrap()
    }

    fn artifact(&self) -> GitProgramArtifact {
        GitProgramArtifact::new(
            BTreeMap::from([(
                repo_key("program"),
                GitRepoArtifact::new(
                    RepoRef::global(repo_key("program")),
                    self.parent.clone(),
                    None,
                    GitArtifactIdentityMode::Commit,
                ),
            )]),
            GitProgramLayout::new(BTreeMap::from([(
                repo_key("program"),
                git_path("repos/program"),
            )]))
            .unwrap(),
        )
        .unwrap()
    }
}

#[derive(Debug)]
struct HostProductPodRuntime {
    guest_root: String,
    pod_root: tempfile::TempDir,
    state: Mutex<HostProductPodState>,
}

impl HostProductPodRuntime {
    fn new(guest_root: &str) -> Self {
        Self {
            guest_root: guest_root.trim_end_matches('/').to_owned(),
            pod_root: tempfile::tempdir().unwrap(),
            state: Mutex::new(HostProductPodState::default()),
        }
    }

    fn host_path(&self, guest_path: &FirkinGuestPath) -> Result<PathBuf, FirkinRuntimeError> {
        let raw = guest_path.as_str();
        let relative = raw
            .strip_prefix(&self.guest_root)
            .ok_or_else(|| FirkinRuntimeError::Runtime {
                operation: "map Firkin guest path",
                reason: format!("{raw} is outside {}", self.guest_root),
            })?
            .trim_start_matches('/');
        Ok(self.pod_root.path().join(relative))
    }

    fn workspace_host_root(
        &self,
        container: &FirkinContainerId,
    ) -> Result<PathBuf, FirkinRuntimeError> {
        let state = self.state.lock().unwrap();
        let root = state.containers.get(container.as_str()).ok_or_else(|| {
            FirkinRuntimeError::Runtime {
                operation: "resolve host Firkin container",
                reason: format!("unknown container `{container}`"),
            }
        })?;
        self.host_path(root)
    }

    fn removed_containers(&self) -> BTreeSet<String> {
        self.state.lock().unwrap().removed.clone()
    }
}

#[derive(Default, Debug)]
struct HostProductPodState {
    next_container: usize,
    containers: BTreeMap<String, FirkinGuestPath>,
    removed: BTreeSet<String>,
}

impl FirkinWorkspaceRuntime for HostProductPodRuntime {
    fn allocate_container(
        &self,
        request: FirkinWorkspaceAllocation,
    ) -> Result<FirkinContainerId, FirkinRuntimeError> {
        let mut state = self.state.lock().unwrap();
        state.next_container += 1;
        let container = format!("container-{}", state.next_container);
        fs::create_dir_all(self.host_path(request.workspace_root())?).map_err(|source| {
            FirkinRuntimeError::Runtime {
                operation: "create host Firkin workspace root",
                reason: source.to_string(),
            }
        })?;
        state
            .containers
            .insert(container.clone(), request.workspace_root().clone());
        FirkinContainerId::new(container)
    }

    fn write_file(
        &self,
        _container: &FirkinContainerId,
        path: &FirkinGuestPath,
        bytes: &[u8],
    ) -> Result<(), FirkinRuntimeError> {
        let path = self.host_path(path)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|source| FirkinRuntimeError::Runtime {
                operation: "create host Firkin file parent",
                reason: source.to_string(),
            })?;
        }
        fs::write(path, bytes).map_err(|source| FirkinRuntimeError::Runtime {
            operation: "write host Firkin file",
            reason: source.to_string(),
        })
    }

    fn read_file(
        &self,
        _container: &FirkinContainerId,
        path: &FirkinGuestPath,
    ) -> Result<Vec<u8>, FirkinRuntimeError> {
        fs::read(self.host_path(path)?).map_err(|source| FirkinRuntimeError::Runtime {
            operation: "read host Firkin file",
            reason: source.to_string(),
        })
    }

    fn list_files(
        &self,
        container: &FirkinContainerId,
        root: &FirkinGuestPath,
    ) -> Result<Vec<WorkspacePath>, FirkinRuntimeError> {
        let workspace_root = self.workspace_host_root(container)?;
        let root = self.host_path(root)?;
        let mut files = Vec::new();
        collect_files(&workspace_root, &root, &mut files)?;
        files.sort();
        Ok(files)
    }

    fn run_command(
        &self,
        _container: &FirkinContainerId,
        request: FirkinCommandRequest,
    ) -> Result<FirkinCommandResult, FirkinRuntimeError> {
        if request.user.is_some() {
            return Err(FirkinRuntimeError::Runtime {
                operation: "run host Firkin command",
                reason: "user override is unsupported in host-backed test runtime".to_owned(),
            });
        }
        let cwd = self.host_path(&request.cwd)?;
        let start = Instant::now();
        let mut process = ProcessCommand::new(&request.program);
        process
            .args(&request.args)
            .current_dir(cwd)
            .envs(&request.env)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        match &request.stdin {
            bytes if bytes.is_empty() => {
                process.stdin(Stdio::null());
            }
            _ => {
                process.stdin(Stdio::piped());
            }
        }
        let mut child = process
            .spawn()
            .map_err(|source| FirkinRuntimeError::Runtime {
                operation: "spawn host Firkin command",
                reason: source.to_string(),
            })?;
        if !request.stdin.is_empty()
            && let Some(mut stdin) = child.stdin.take()
        {
            stdin
                .write_all(&request.stdin)
                .map_err(|source| FirkinRuntimeError::Runtime {
                    operation: "write host Firkin command stdin",
                    reason: source.to_string(),
                })?;
        }
        let output = child
            .wait_with_output()
            .map_err(|source| FirkinRuntimeError::Runtime {
                operation: "wait host Firkin command",
                reason: source.to_string(),
            })?;
        Ok(FirkinCommandResult {
            status: ExitStatus {
                code: output.status.code(),
            },
            stdout: CapturedOutput::new(output.stdout, request.max_stdout_bytes),
            stderr: CapturedOutput::new(output.stderr, request.max_stderr_bytes),
            duration: start.elapsed(),
        })
    }

    fn remove_container(&self, container: FirkinContainerId) -> Result<(), FirkinRuntimeError> {
        let mut state = self.state.lock().unwrap();
        state.containers.remove(container.as_str());
        state.removed.insert(container.as_str().to_owned());
        Ok(())
    }
}

fn collect_files(
    workspace_root: &Path,
    host_path: &Path,
    files: &mut Vec<WorkspacePath>,
) -> Result<(), FirkinRuntimeError> {
    let metadata = fs::metadata(host_path).map_err(|source| FirkinRuntimeError::Runtime {
        operation: "stat host Firkin file",
        reason: source.to_string(),
    })?;
    if metadata.is_file() {
        let relative = host_path
            .strip_prefix(workspace_root)
            .map_err(|source| FirkinRuntimeError::Runtime {
                operation: "relativize host Firkin file",
                reason: source.to_string(),
            })?
            .to_string_lossy()
            .replace(std::path::MAIN_SEPARATOR, "/");
        files.push(WorkspacePath::new(relative)?);
        return Ok(());
    }
    for entry in fs::read_dir(host_path).map_err(|source| FirkinRuntimeError::Runtime {
        operation: "read host Firkin directory",
        reason: source.to_string(),
    })? {
        let entry = entry.map_err(|source| FirkinRuntimeError::Runtime {
            operation: "read host Firkin directory entry",
            reason: source.to_string(),
        })?;
        collect_files(workspace_root, &entry.path(), files)?;
    }
    Ok(())
}

fn create_repo(root: &Path) {
    fs::create_dir_all(root).unwrap();
    run_git(root, ["init", "--initial-branch=main"]);
    run_git(root, ["config", "user.name", "Leaven Test"]);
    run_git(root, ["config", "user.email", "leaven@example.invalid"]);
    fs::write(root.join("program.txt"), "program base\n").unwrap();
    run_git(root, ["add", "program.txt"]);
    run_git(root, ["commit", "-m", "base"]);
}

fn run_git<const N: usize>(cwd: &Path, args: [&str; N]) {
    run_git_at(cwd, args);
}

fn run_git_at<const N: usize>(cwd: &Path, args: [&str; N]) {
    let output = ProcessCommand::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {} failed: {}",
        args.join(" "),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn git_output<const N: usize>(cwd: &Path, args: [&str; N]) -> String {
    let output = ProcessCommand::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {} failed: {}",
        args.join(" "),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

fn repo_key(key: &str) -> RepoKey {
    RepoKey::new(key).unwrap()
}

fn git_path(path: &str) -> GitPath {
    GitPath::new(path).unwrap()
}

fn workspace_path(path: &str) -> WorkspacePath {
    WorkspacePath::new(path).unwrap()
}

fn git_object(cwd: &Path, rev: &str) -> GitObjectId {
    GitObjectId::new(git_output(cwd, ["rev-parse", rev]).trim()).unwrap()
}
