use leaven_kernel::RunId;
use std::collections::BTreeMap;
use std::time::Duration;

use leaven_workspace::{
    Command, CommandLimits, CommandStdin, CommandUser, WorkspaceConfig, WorkspaceFactory,
    WorkspacePath,
};
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
                env: BTreeMap::new(),
                stdin: CommandStdin::Empty,
                limits: CommandLimits::default(),
                user: None,
            })
            .unwrap();

        assert_eq!(output.status.code, Some(0));
        assert_eq!(output.stdout.bytes, b"hello");
        assert!(!output.stdout.truncated);
        assert!(output.stderr.bytes.is_empty());
        assert!(!output.stderr.truncated);
        assert!(output.duration >= Duration::ZERO);

        drop(view);
        workspace.cleanup().await.unwrap();
        assert!(!mount.exists());
        remove_dir(&parent);
    });
}

#[test]
fn local_workspace_passes_env_and_stdin_to_commands() {
    futures::executor::block_on(async {
        let parent = temp_parent("local-command-env-stdin");
        let factory = LocalWorkspaceFactory::new(&parent);
        let mut workspace = factory.allocate(WorkspaceConfig::default()).await.unwrap();
        let mut view = workspace.view();

        let output = view
            .run_command(Command {
                program: "sh".to_owned(),
                args: vec![
                    "-c".to_owned(),
                    "printf '%s:' \"$LEAVEN_TEST_VALUE\"; cat".to_owned(),
                ],
                cwd: None,
                env: BTreeMap::from([("LEAVEN_TEST_VALUE".to_owned(), "from-env".to_owned())]),
                stdin: CommandStdin::Bytes(b"from-stdin".to_vec()),
                limits: CommandLimits::default(),
                user: None,
            })
            .unwrap();

        assert_eq!(output.status.code, Some(0));
        assert_eq!(output.stdout.bytes, b"from-env:from-stdin");
        assert!(!output.stdout.truncated);
        assert!(output.stderr.bytes.is_empty());
        assert!(output.duration >= Duration::ZERO);

        drop(view);
        workspace.cleanup().await.unwrap();
        remove_dir(&parent);
    });
}

#[test]
fn local_workspace_truncates_stdout_and_stderr_independently() {
    futures::executor::block_on(async {
        let parent = temp_parent("local-command-truncate");
        let factory = LocalWorkspaceFactory::new(&parent);
        let mut workspace = factory.allocate(WorkspaceConfig::default()).await.unwrap();
        let mut view = workspace.view();

        let output = view
            .run_command(Command {
                program: "sh".to_owned(),
                args: vec![
                    "-c".to_owned(),
                    "printf stdout-long; printf stderr-long >&2".to_owned(),
                ],
                cwd: None,
                env: BTreeMap::new(),
                stdin: CommandStdin::Empty,
                limits: CommandLimits {
                    timeout: None,
                    max_stdout_bytes: Some(6),
                    max_stderr_bytes: Some(6),
                },
                user: None,
            })
            .unwrap();

        assert_eq!(output.status.code, Some(0));
        assert_eq!(output.stdout.bytes, b"stdout");
        assert!(output.stdout.truncated);
        assert_eq!(output.stderr.bytes, b"stderr");
        assert!(output.stderr.truncated);

        drop(view);
        workspace.cleanup().await.unwrap();
        remove_dir(&parent);
    });
}

#[test]
fn local_workspace_refuses_command_user_instead_of_ignoring_it() {
    futures::executor::block_on(async {
        let parent = temp_parent("local-command-user");
        let factory = LocalWorkspaceFactory::new(&parent);
        let mut workspace = factory.allocate(WorkspaceConfig::default()).await.unwrap();
        let mut view = workspace.view();

        let error = view
            .run_command(Command {
                program: "true".to_owned(),
                args: Vec::new(),
                cwd: None,
                env: BTreeMap::new(),
                stdin: CommandStdin::Empty,
                limits: CommandLimits::default(),
                user: Some(CommandUser::Name("nobody".to_owned())),
            })
            .unwrap_err();

        assert!(matches!(
            error,
            leaven_workspace::WorkspaceError::UnsupportedOperation {
                operation: "run_command.user"
            }
        ));

        drop(view);
        workspace.cleanup().await.unwrap();
        remove_dir(&parent);
    });
}

#[test]
fn local_workspace_times_out_commands_without_hanging() {
    futures::executor::block_on(async {
        let parent = temp_parent("local-command-timeout");
        let factory = LocalWorkspaceFactory::new(&parent);
        let mut workspace = factory.allocate(WorkspaceConfig::default()).await.unwrap();
        let mut view = workspace.view();

        let error = view
            .run_command(Command {
                program: "sh".to_owned(),
                args: vec!["-c".to_owned(), "sleep 2; printf done".to_owned()],
                cwd: None,
                env: BTreeMap::new(),
                stdin: CommandStdin::Empty,
                limits: CommandLimits {
                    timeout: Some(Duration::from_millis(50)),
                    max_stdout_bytes: None,
                    max_stderr_bytes: None,
                },
                user: None,
            })
            .unwrap_err();

        assert!(matches!(
            error,
            leaven_workspace::WorkspaceError::CommandTimedOut { .. }
        ));

        drop(view);
        workspace.cleanup().await.unwrap();
        remove_dir(&parent);
    });
}

#[test]
fn local_workspace_can_toggle_executable_permissions_off_again() {
    futures::executor::block_on(async {
        let parent = temp_parent("local-executable-toggle");
        let factory = LocalWorkspaceFactory::new(&parent);
        let mut workspace = factory.allocate(WorkspaceConfig::default()).await.unwrap();
        let script = WorkspacePath::new("scripts/run.sh").unwrap();
        let mut view = workspace.view();
        view.write_file(&script, b"#!/bin/sh\nexit 0\n").unwrap();

        view.set_executable(&script, true).unwrap();
        assert!(view.is_executable(&script).unwrap());
        view.set_executable(&script, false).unwrap();
        assert!(!view.is_executable(&script).unwrap());

        drop(view);
        workspace.cleanup().await.unwrap();
        remove_dir(&parent);
    });
}

#[test]
fn local_workspace_lists_recursive_files_from_root_and_subdir() {
    futures::executor::block_on(async {
        let parent = temp_parent("local-list-files");
        let factory = LocalWorkspaceFactory::new(&parent);
        let mut workspace = factory.allocate(WorkspaceConfig::default()).await.unwrap();
        let mut view = workspace.view();
        view.write_file(&WorkspacePath::new("alpha/root.txt").unwrap(), b"root")
            .unwrap();
        view.write_file(
            &WorkspacePath::new("alpha/nested/leaf.txt").unwrap(),
            b"leaf",
        )
        .unwrap();
        view.write_file(&WorkspacePath::new("beta.txt").unwrap(), b"beta")
            .unwrap();

        assert_eq!(
            view.list_files(&WorkspacePath::root()).unwrap(),
            vec![
                WorkspacePath::new("alpha/nested/leaf.txt").unwrap(),
                WorkspacePath::new("alpha/root.txt").unwrap(),
                WorkspacePath::new("beta.txt").unwrap(),
            ]
        );
        assert_eq!(
            view.list_files(&WorkspacePath::new("alpha").unwrap())
                .unwrap(),
            vec![
                WorkspacePath::new("alpha/nested/leaf.txt").unwrap(),
                WorkspacePath::new("alpha/root.txt").unwrap(),
            ]
        );

        drop(view);
        workspace.cleanup().await.unwrap();
        remove_dir(&parent);
    });
}

#[cfg(unix)]
#[test]
fn local_workspace_file_apis_refuse_symlink_escape() {
    futures::executor::block_on(async {
        let parent = temp_parent("local-symlink-escape");
        let outside = temp_parent("local-symlink-outside");
        std::fs::write(outside.join("outside.txt"), b"outside").unwrap();
        let factory = LocalWorkspaceFactory::new(&parent);
        let mut workspace = factory.allocate(WorkspaceConfig::default()).await.unwrap();
        let mount = workspace.local_mount().unwrap().to_path_buf();
        std::os::unix::fs::symlink(outside.join("outside.txt"), mount.join("linked-file")).unwrap();
        std::os::unix::fs::symlink(&outside, mount.join("linked-dir")).unwrap();

        let linked_file = WorkspacePath::new("linked-file").unwrap();
        let linked_dir = WorkspacePath::new("linked-dir").unwrap();
        let mut view = workspace.view();
        let files = view.list_files(&WorkspacePath::root()).unwrap();
        assert!(files.contains(&linked_file));
        assert!(files.contains(&linked_dir));
        assert!(!files.contains(&WorkspacePath::new("linked-dir/outside.txt").unwrap()));

        assert!(matches!(
            view.read_file(&linked_file).unwrap_err(),
            leaven_workspace::WorkspaceError::Io(_)
        ));
        assert!(matches!(
            view.write_file(&linked_file, b"changed").unwrap_err(),
            leaven_workspace::WorkspaceError::Io(_)
        ));
        assert_eq!(
            std::fs::read(outside.join("outside.txt")).unwrap(),
            b"outside"
        );
        assert!(matches!(
            view.set_executable(&linked_file, true).unwrap_err(),
            leaven_workspace::WorkspaceError::Io(_)
        ));
        assert!(matches!(
            view.is_executable(&linked_file).unwrap_err(),
            leaven_workspace::WorkspaceError::Io(_)
        ));

        let mut command = Command::new("true");
        command.cwd = Some(linked_dir);
        assert!(matches!(
            view.run_command(command).unwrap_err(),
            leaven_workspace::WorkspaceError::Io(_)
        ));

        drop(view);
        workspace.cleanup().await.unwrap();
        remove_dir(&parent);
        remove_dir(&outside);
    });
}

#[test]
fn local_workspace_write_fails_when_parent_path_is_file() {
    futures::executor::block_on(async {
        let parent = temp_parent("local-file-parent");
        let factory = LocalWorkspaceFactory::new(&parent);
        let mut workspace = factory.allocate(WorkspaceConfig::default()).await.unwrap();
        let mut view = workspace.view();
        view.write_file(&WorkspacePath::new("blocked").unwrap(), b"file")
            .unwrap();

        let error = view
            .write_file(&WorkspacePath::new("blocked/child.txt").unwrap(), b"child")
            .unwrap_err();

        assert!(matches!(error, leaven_workspace::WorkspaceError::Io(_)));
        drop(view);
        workspace.cleanup().await.unwrap();
        remove_dir(&parent);
    });
}

#[cfg(unix)]
#[test]
fn local_workspace_write_reports_directory_creation_failure() {
    use std::os::unix::fs::PermissionsExt;

    futures::executor::block_on(async {
        let parent = temp_parent("local-readonly-parent");
        let factory = LocalWorkspaceFactory::new(&parent);
        let mut workspace = factory.allocate(WorkspaceConfig::default()).await.unwrap();
        let mount = workspace.local_mount().unwrap().to_path_buf();
        let mut permissions = std::fs::metadata(&mount).unwrap().permissions();
        permissions.set_mode(0o500);
        std::fs::set_permissions(&mount, permissions).unwrap();

        let error = workspace
            .view()
            .write_file(&WorkspacePath::new("blocked/child.txt").unwrap(), b"child")
            .unwrap_err();

        assert!(matches!(error, leaven_workspace::WorkspaceError::Io(_)));
        let mut permissions = std::fs::metadata(&mount).unwrap().permissions();
        permissions.set_mode(0o700);
        std::fs::set_permissions(&mount, permissions).unwrap();
        workspace.cleanup().await.unwrap();
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
