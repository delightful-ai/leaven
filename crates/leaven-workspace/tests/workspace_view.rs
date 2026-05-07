use std::path::{Path, PathBuf};

use futures::future::{BoxFuture, FutureExt};
use leaven_kernel::RunId;
use leaven_workspace::{
    Command, Workspace, WorkspaceBackend, WorkspaceError, WorkspacePath, WorkspaceView,
};

#[test]
fn workspace_view_writes_reads_and_scopes_subdirectories() {
    let root = temp_root("view-scopes");
    let mut view = WorkspaceView::new(&root);

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
    assert_eq!(
        view.read_file(&WorkspacePath::new("history/visible/evidence/ref.txt").unwrap())
            .unwrap(),
        b"evidence-ref"
    );
    assert_eq!(nested.root().as_str(), "history/visible");
    assert_eq!(deeper.root().as_str(), "history/visible/evidence");

    remove_dir(&root);
}

#[test]
fn workspace_view_refuses_commands_without_attached_backend() {
    let root = temp_root("view-command");
    let mut view = WorkspaceView::new(&root);

    let error = view
        .run_command(Command {
            program: "true".to_owned(),
            args: Vec::new(),
            cwd: None,
        })
        .unwrap_err();

    assert!(matches!(error, WorkspaceError::Command(_)));
    remove_dir(&root);
}

#[test]
fn workspace_lifecycle_exposes_root_mount_view_and_cleanup_backend() {
    let root = temp_root("workspace-lifecycle");
    let mut workspace = Workspace::new(
        root.clone(),
        Box::new(MountBackend {
            mount: root.clone(),
            cleanup_root: root.clone(),
        }),
    );

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
    let workspace = Workspace::new(
        root.clone(),
        Box::new(NoMountBackend { root: root.clone() }),
    );

    assert_eq!(workspace.local_mount(), None);

    futures::executor::block_on(workspace.cleanup()).unwrap();
    assert!(!root.exists());
}

struct MountBackend {
    mount: PathBuf,
    cleanup_root: PathBuf,
}

impl WorkspaceBackend for MountBackend {
    fn cleanup(self: Box<Self>) -> BoxFuture<'static, Result<(), WorkspaceError>> {
        async move {
            remove_dir(&self.cleanup_root);
            Ok(())
        }
        .boxed()
    }

    fn local_mount(&self) -> Option<&Path> {
        Some(&self.mount)
    }
}

struct NoMountBackend {
    root: PathBuf,
}

impl WorkspaceBackend for NoMountBackend {
    fn cleanup(self: Box<Self>) -> BoxFuture<'static, Result<(), WorkspaceError>> {
        async move {
            remove_dir(&self.root);
            Ok(())
        }
        .boxed()
    }
}

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
