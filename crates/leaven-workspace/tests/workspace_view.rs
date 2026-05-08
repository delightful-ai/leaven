use std::path::{Path, PathBuf};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicUsize, Ordering},
};

use futures::future::{BoxFuture, FutureExt};
use leaven_kernel::RunId;
use leaven_workspace::{
    Command, CommandOutput, ExitStatus, FactoryError, WithWorkspaceError, Workspace,
    WorkspaceBackend, WorkspaceConfig, WorkspaceError, WorkspaceFactory, WorkspacePath,
    with_workspace,
};

#[test]
fn workspace_view_writes_reads_and_scopes_subdirectories() {
    let root = temp_root("view-scopes");
    let mut workspace = Workspace::new(root.clone(), Box::new(TestBackend::mounted(&root)));
    let mut view = workspace.view();

    view.write_file(&WorkspacePath::new("artifact/root.txt").unwrap(), b"root")
        .unwrap();
    let mut nested = view
        .subdir(WorkspacePath::new("history/visible").unwrap())
        .unwrap();
    let mut deeper = nested
        .subdir(WorkspacePath::new("evidence").unwrap())
        .unwrap();
    nested
        .write_file(&WorkspacePath::new("candidate.txt").unwrap(), b"candidate")
        .unwrap();
    deeper
        .write_file(&WorkspacePath::new("ref.txt").unwrap(), b"evidence-ref")
        .unwrap();

    assert_eq!(
        view.read_file(&WorkspacePath::new("artifact/root.txt").unwrap())
            .unwrap(),
        b"root"
    );
    assert_eq!(
        view.list_files(&WorkspacePath::root()).unwrap(),
        vec![
            WorkspacePath::new("artifact/root.txt").unwrap(),
            WorkspacePath::new("history/visible/candidate.txt").unwrap(),
            WorkspacePath::new("history/visible/evidence/ref.txt").unwrap(),
        ]
    );
    assert_eq!(
        nested.list_files(&WorkspacePath::root()).unwrap(),
        vec![
            WorkspacePath::new("candidate.txt").unwrap(),
            WorkspacePath::new("evidence/ref.txt").unwrap(),
        ]
    );
    assert_eq!(
        view.read_file(&WorkspacePath::new("history/visible/candidate.txt").unwrap())
            .unwrap(),
        b"candidate"
    );
    assert_eq!(
        nested
            .read_file(&WorkspacePath::new("candidate.txt").unwrap())
            .unwrap(),
        b"candidate"
    );
    assert!(nested.read_file(&WorkspacePath::root()).is_err());
    assert_eq!(
        view.read_file(&WorkspacePath::new("history/visible/evidence/ref.txt").unwrap())
            .unwrap(),
        b"evidence-ref"
    );
    assert_eq!(nested.root().as_str(), "history/visible");
    assert_eq!(deeper.root().as_str(), "history/visible/evidence");

    drop(deeper);
    drop(nested);
    drop(view);
    futures::executor::block_on(workspace.cleanup()).unwrap();
    remove_dir(&root);
}

#[test]
fn workspace_view_delegates_commands_to_backend_with_scoped_cwd() {
    let root = temp_root("view-command");
    let commands = Arc::new(Mutex::new(Vec::new()));
    let mut workspace = Workspace::new(
        root.clone(),
        Box::new(TestBackend::mounted_with_commands(&root, commands.clone())),
    );
    let mut view = workspace
        .view()
        .subdir(WorkspacePath::new("candidate").unwrap())
        .unwrap();

    let output = view
        .run_command(Command {
            program: "echo".to_owned(),
            args: vec!["ok".to_owned()],
            cwd: Some(WorkspacePath::new("work").unwrap()),
        })
        .unwrap();

    assert_eq!(output.status.code, Some(0));
    assert_eq!(output.stdout, b"ok");
    let recorded = commands.lock().unwrap();
    assert_eq!(recorded.len(), 1);
    assert_eq!(recorded[0].cwd.as_ref().unwrap().as_str(), "candidate/work");
    drop(recorded);

    let output = view
        .run_command(Command {
            program: "pwd".to_owned(),
            args: Vec::new(),
            cwd: None,
        })
        .unwrap();

    assert_eq!(output.status.code, Some(0));
    let recorded = commands.lock().unwrap();
    assert_eq!(recorded.len(), 2);
    assert_eq!(recorded[1].cwd.as_ref().unwrap().as_str(), "candidate");
    drop(recorded);

    drop(view);
    futures::executor::block_on(workspace.cleanup()).unwrap();
    remove_dir(&root);
}

#[test]
fn workspace_lifecycle_exposes_root_mount_view_and_cleanup_backend() {
    let root = temp_root("workspace-lifecycle");
    let mut workspace = Workspace::new(root.clone(), Box::new(TestBackend::mounted(&root)));

    assert_eq!(workspace.root().as_str(), "");
    assert_eq!(workspace.local_mount(), Some(root.as_path()));
    workspace
        .view()
        .write_file(&WorkspacePath::new("out.txt").unwrap(), b"ok")
        .unwrap();
    assert_eq!(std::fs::read(root.join("out.txt")).unwrap(), b"ok");

    futures::executor::block_on(workspace.cleanup()).unwrap();

    assert!(!root.exists());
}

#[test]
fn workspace_backend_local_mount_defaults_to_none() {
    let root = temp_root("workspace-no-mount");
    let mut workspace = Workspace::new(root.clone(), Box::new(TestBackend::unmounted(&root)));

    assert_eq!(workspace.local_mount(), None);
    workspace
        .view()
        .write_file(&WorkspacePath::new("remote-only.txt").unwrap(), b"ok")
        .unwrap();
    assert_eq!(
        workspace
            .view()
            .read_file(&WorkspacePath::new("remote-only.txt").unwrap())
            .unwrap(),
        b"ok"
    );

    futures::executor::block_on(workspace.cleanup()).unwrap();
    assert!(!root.exists());
}

#[test]
fn workspace_backend_default_operations_are_explicitly_unsupported() {
    let mut workspace = Workspace::new(PathBuf::new(), Box::new(UnsupportedBackend));
    let mut view = workspace.view();

    assert!(matches!(
        view.write_file(&WorkspacePath::new("out.txt").unwrap(), b"no"),
        Err(WorkspaceError::UnsupportedOperation {
            operation: "write_file"
        })
    ));
    assert!(matches!(
        view.read_file(&WorkspacePath::new("out.txt").unwrap()),
        Err(WorkspaceError::UnsupportedOperation {
            operation: "read_file"
        })
    ));
    assert!(matches!(
        view.list_files(&WorkspacePath::root()),
        Err(WorkspaceError::UnsupportedOperation {
            operation: "list_files"
        })
    ));
    assert!(matches!(
        view.run_command(Command {
            program: "true".to_owned(),
            args: Vec::new(),
            cwd: None,
        }),
        Err(WorkspaceError::UnsupportedOperation {
            operation: "run_command"
        })
    ));

    drop(view);
    futures::executor::block_on(workspace.cleanup()).unwrap();
}

#[test]
fn with_workspace_returns_success_value_after_cleanup() {
    let root = temp_root("with-workspace-success");
    let cleanup_count = Arc::new(AtomicUsize::new(0));
    let factory = TestFactory {
        root: root.clone(),
        cleanup_count: cleanup_count.clone(),
        cleanup_error: None,
    };

    let value = futures::executor::block_on(with_workspace(
        &factory,
        WorkspaceConfig::default(),
        |workspace| {
            async move {
                workspace
                    .view()
                    .write_file(&WorkspacePath::new("ok.txt").unwrap(), b"ok")
                    .unwrap();
                Ok::<_, StageError>(42)
            }
            .boxed()
        },
    ))
    .unwrap();

    assert_eq!(value, 42);
    assert_eq!(cleanup_count.load(Ordering::SeqCst), 1);
    assert!(!root.exists());
}

#[test]
fn with_workspace_cleans_up_after_stage_error() {
    let root = temp_root("with-workspace-stage-error");
    let cleanup_count = Arc::new(AtomicUsize::new(0));
    let factory = TestFactory {
        root: root.clone(),
        cleanup_count: cleanup_count.clone(),
        cleanup_error: None,
    };

    let error = futures::executor::block_on(with_workspace(
        &factory,
        WorkspaceConfig::default(),
        |workspace| {
            async move {
                workspace
                    .view()
                    .write_file(&WorkspacePath::new("before-error.txt").unwrap(), b"ok")
                    .unwrap();
                Err::<(), StageError>(StageError("stage failed"))
            }
            .boxed()
        },
    ))
    .unwrap_err();

    assert!(matches!(
        error,
        WithWorkspaceError::Stage(StageError("stage failed"))
    ));
    assert_eq!(cleanup_count.load(Ordering::SeqCst), 1);
    assert!(!root.exists());
}

#[test]
fn with_workspace_reports_cleanup_failure_after_success() {
    let root = temp_root("with-workspace-cleanup-error");
    let cleanup_count = Arc::new(AtomicUsize::new(0));
    let factory = TestFactory {
        root: root.clone(),
        cleanup_count: cleanup_count.clone(),
        cleanup_error: Some("cleanup failed"),
    };

    let error = futures::executor::block_on(with_workspace(
        &factory,
        WorkspaceConfig::default(),
        |_workspace| async { Ok::<_, StageError>(()) }.boxed(),
    ))
    .unwrap_err();

    assert!(matches!(
        error,
        WithWorkspaceError::Cleanup(WorkspaceError::Cleanup(_))
    ));
    assert_eq!(cleanup_count.load(Ordering::SeqCst), 1);
    remove_dir(&root);
}

#[test]
fn with_workspace_preserves_stage_and_cleanup_failures() {
    let root = temp_root("with-workspace-double-error");
    let cleanup_count = Arc::new(AtomicUsize::new(0));
    let factory = TestFactory {
        root: root.clone(),
        cleanup_count: cleanup_count.clone(),
        cleanup_error: Some("cleanup failed"),
    };

    let error = futures::executor::block_on(with_workspace(
        &factory,
        WorkspaceConfig::default(),
        |_workspace| async { Err::<(), StageError>(StageError("stage failed")) }.boxed(),
    ))
    .unwrap_err();

    assert!(matches!(
        error,
        WithWorkspaceError::StageAndCleanup {
            stage: StageError("stage failed"),
            cleanup: WorkspaceError::Cleanup(_)
        }
    ));
    assert_eq!(cleanup_count.load(Ordering::SeqCst), 1);
    remove_dir(&root);
}

struct TestBackend {
    root: PathBuf,
    mount: Option<PathBuf>,
    commands: Arc<Mutex<Vec<Command>>>,
    cleanup_count: Option<Arc<AtomicUsize>>,
    cleanup_error: Option<&'static str>,
}

impl TestBackend {
    fn mounted(root: &Path) -> Self {
        Self::mounted_with_commands(root, Arc::new(Mutex::new(Vec::new())))
    }

    fn mounted_with_commands(root: &Path, commands: Arc<Mutex<Vec<Command>>>) -> Self {
        Self {
            root: root.to_path_buf(),
            mount: Some(root.to_path_buf()),
            commands,
            cleanup_count: None,
            cleanup_error: None,
        }
    }

    fn unmounted(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
            mount: None,
            commands: Arc::new(Mutex::new(Vec::new())),
            cleanup_count: None,
            cleanup_error: None,
        }
    }

    fn with_cleanup(
        root: PathBuf,
        cleanup_count: Arc<AtomicUsize>,
        cleanup_error: Option<&'static str>,
    ) -> Self {
        Self {
            root,
            mount: None,
            commands: Arc::new(Mutex::new(Vec::new())),
            cleanup_count: Some(cleanup_count),
            cleanup_error,
        }
    }

    fn host_path(&self, path: &WorkspacePath) -> PathBuf {
        self.root.join(path.to_host_relative())
    }
}

impl WorkspaceBackend for TestBackend {
    fn write_file(&mut self, path: &WorkspacePath, bytes: &[u8]) -> Result<(), WorkspaceError> {
        let path = self.host_path(path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|err| WorkspaceError::Io(err.to_string()))?;
        }
        std::fs::write(path, bytes).map_err(|err| WorkspaceError::Io(err.to_string()))
    }

    fn read_file(&mut self, path: &WorkspacePath) -> Result<Vec<u8>, WorkspaceError> {
        std::fs::read(self.host_path(path)).map_err(|err| WorkspaceError::Io(err.to_string()))
    }

    fn list_files(&mut self, path: &WorkspacePath) -> Result<Vec<WorkspacePath>, WorkspaceError> {
        let root = self.host_path(path);
        let mut files = Vec::new();
        collect_files(&root, path.clone(), &mut files)?;
        files.sort();
        Ok(files)
    }

    fn run_command(&mut self, command: Command) -> Result<CommandOutput, WorkspaceError> {
        self.commands.lock().unwrap().push(command);
        Ok(CommandOutput {
            status: ExitStatus { code: Some(0) },
            stdout: b"ok".to_vec(),
            stderr: Vec::new(),
        })
    }

    fn cleanup(self: Box<Self>) -> BoxFuture<'static, Result<(), WorkspaceError>> {
        async move {
            if let Some(cleanup_count) = &self.cleanup_count {
                cleanup_count.fetch_add(1, Ordering::SeqCst);
            }
            if let Some(message) = self.cleanup_error {
                return Err(WorkspaceError::Cleanup(message.to_owned()));
            }
            remove_dir(&self.root);
            Ok(())
        }
        .boxed()
    }

    fn local_mount(&self) -> Option<&Path> {
        self.mount.as_deref()
    }
}

struct TestFactory {
    root: PathBuf,
    cleanup_count: Arc<AtomicUsize>,
    cleanup_error: Option<&'static str>,
}

struct UnsupportedBackend;

impl WorkspaceBackend for UnsupportedBackend {
    fn cleanup(self: Box<Self>) -> BoxFuture<'static, Result<(), WorkspaceError>> {
        async { Ok(()) }.boxed()
    }
}

impl WorkspaceFactory for TestFactory {
    async fn allocate(&self, _config: WorkspaceConfig) -> Result<Workspace, FactoryError> {
        Ok(Workspace::new(
            self.root.clone(),
            Box::new(TestBackend::with_cleanup(
                self.root.clone(),
                self.cleanup_count.clone(),
                self.cleanup_error,
            )),
        ))
    }
}

#[derive(Debug, Eq, PartialEq, thiserror::Error)]
#[error("{0}")]
struct StageError(&'static str);

fn temp_root(label: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("leaven-{label}-{}", RunId::new()));
    remove_dir(&root);
    std::fs::create_dir_all(&root).unwrap();
    root
}

fn remove_dir(path: &Path) {
    if path.exists() {
        std::fs::remove_dir_all(path).unwrap();
    }
}

fn collect_files(
    host_path: &Path,
    workspace_path: WorkspacePath,
    files: &mut Vec<WorkspacePath>,
) -> Result<(), WorkspaceError> {
    let metadata =
        std::fs::metadata(host_path).map_err(|err| WorkspaceError::Io(err.to_string()))?;
    if metadata.is_file() {
        files.push(workspace_path);
        return Ok(());
    }
    for entry in std::fs::read_dir(host_path).map_err(|err| WorkspaceError::Io(err.to_string()))? {
        let entry = entry.map_err(|err| WorkspaceError::Io(err.to_string()))?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| WorkspaceError::Io("workspace path is not UTF-8".to_owned()))?;
        let child_path = if workspace_path.as_str().is_empty() {
            WorkspacePath::new(name)?
        } else {
            workspace_path.join(name)?
        };
        collect_files(&entry.path(), child_path, files)?;
    }
    Ok(())
}
