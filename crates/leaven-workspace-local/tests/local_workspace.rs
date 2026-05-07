use leaven_kernel::RunId;
use leaven_workspace::{Command, WorkspaceConfig, WorkspaceFactory, WorkspacePath};
use leaven_workspace_local::LocalWorkspaceFactory;

#[test]
fn local_workspace_factory_allocates_mounts_and_cleanup_removes_them() {
    futures::executor::block_on(async {
        let parent = temp_parent("local-workspace");
        let factory = LocalWorkspaceFactory::new(&parent);
        let mut workspace = factory.allocate(WorkspaceConfig::default()).await.unwrap();
        let mount = workspace
            .local_mount()
            .expect("local mount is available")
            .to_path_buf();

        assert!(mount.starts_with(&parent));
        assert!(mount.exists());

        workspace
            .view()
            .write_file(&WorkspacePath::new("artifact/harness.py").unwrap(), b"pass")
            .unwrap();
        assert_eq!(
            std::fs::read(mount.join("artifact/harness.py")).unwrap(),
            b"pass"
        );

        workspace.cleanup().await.unwrap();

        assert!(!mount.exists());
        remove_dir(&parent);
    });
}

#[test]
fn local_workspace_factory_allocates_unique_roots() {
    futures::executor::block_on(async {
        let parent = temp_parent("local-unique");
        let factory = LocalWorkspaceFactory::new(&parent);
        let first = factory.allocate(WorkspaceConfig::default()).await.unwrap();
        let second = factory.allocate(WorkspaceConfig::default()).await.unwrap();
        let first_mount = first.local_mount().unwrap().to_path_buf();
        let second_mount = second.local_mount().unwrap().to_path_buf();

        assert_ne!(first_mount, second_mount);
        assert!(first_mount.exists());
        assert!(second_mount.exists());

        first.cleanup().await.unwrap();
        second.cleanup().await.unwrap();
        remove_dir(&parent);
    });
}

#[test]
fn default_factory_uses_process_temp_parent() {
    futures::executor::block_on(async {
        let factory = LocalWorkspaceFactory::default();
        let workspace = factory.allocate(WorkspaceConfig::default()).await.unwrap();
        let mount = workspace.local_mount().unwrap().to_path_buf();

        assert!(mount.starts_with(std::env::temp_dir()));

        workspace.cleanup().await.unwrap();
    });
}

#[test]
fn cleanup_succeeds_when_mount_was_already_removed() {
    futures::executor::block_on(async {
        let parent = temp_parent("local-already-removed");
        let factory = LocalWorkspaceFactory::new(&parent);
        let workspace = factory.allocate(WorkspaceConfig::default()).await.unwrap();
        let mount = workspace.local_mount().unwrap().to_path_buf();

        remove_dir(&mount);

        workspace.cleanup().await.unwrap();
        remove_dir(&parent);
    });
}

#[test]
fn local_workspace_runs_commands_inside_scoped_workspace_paths() {
    futures::executor::block_on(async {
        let parent = temp_parent("local-command");
        let factory = LocalWorkspaceFactory::new(&parent);
        let mut workspace = factory.allocate(WorkspaceConfig::default()).await.unwrap();
        let mount = workspace.local_mount().unwrap().to_path_buf();
        let mut view = workspace.view();
        view.write_file(&WorkspacePath::new("work/input.txt").unwrap(), b"hello")
            .unwrap();

        let output = view
            .run_command(Command {
                program: "cat".to_owned(),
                args: vec!["input.txt".to_owned()],
                cwd: Some(WorkspacePath::new("work").unwrap()),
            })
            .unwrap();

        assert_eq!(output.status.code, Some(0));
        assert_eq!(output.stdout, b"hello");
        assert!(output.stderr.is_empty());

        drop(view);
        workspace.cleanup().await.unwrap();
        assert!(!mount.exists());
        remove_dir(&parent);
    });
}

fn temp_parent(label: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!("leaven-{label}-{}", RunId::new()));
    remove_dir(&root);
    std::fs::create_dir_all(&root).unwrap();
    root
}

fn remove_dir(path: &std::path::Path) {
    if path.exists() {
        std::fs::remove_dir_all(path).unwrap();
    }
}
