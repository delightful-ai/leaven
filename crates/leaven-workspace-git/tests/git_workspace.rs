use std::fs;
use std::time::Duration;

use futures::executor::block_on;
use leaven_workspace::{Command, WorkspaceConfig, WorkspaceFactory, WorkspacePath};
use leaven_workspace_git::{GitCheckout, GitWorkspaceFactory};

#[path = "support/git.rs"]
mod git_support;

use git_support::{
    branch_key, checked_out_ref, run_git, run_git_with_identity, tag_key, workspace_fixture_repo,
};

#[test]
fn git_workspace_factory_clones_and_checks_out_program_branch() {
    block_on(async {
        let source = workspace_fixture_repo();
        let factory = GitWorkspaceFactory::local(source.path()).with_checkout("program/base");
        let mut workspace = factory.allocate(WorkspaceConfig::default()).await.unwrap();
        let mount = workspace
            .local_mount()
            .expect("git workspace has local mount")
            .to_path_buf();
        assert!(mount.join(".git").exists());

        {
            let mut slot = workspace.slot(WorkspacePath::root()).unwrap();
            assert_eq!(
                slot.read_file(&WorkspacePath::new("program.txt").unwrap())
                    .unwrap(),
                b"program branch\n"
            );
            let mut command = Command::new("git");
            command.args = vec![
                "rev-parse".to_owned(),
                "--abbrev-ref".to_owned(),
                "HEAD".to_owned(),
            ];
            let output = slot.run_command(command).unwrap();
            assert_eq!(output.status.code, Some(0));
            assert_eq!(
                String::from_utf8(output.stdout.bytes).unwrap().trim(),
                "program/base"
            );
        }

        workspace.cleanup().await.unwrap();
        assert!(!mount.exists());
    });
}

#[test]
fn git_workspace_checkout_failure_removes_allocated_clone() {
    block_on(async {
        let source = workspace_fixture_repo();
        let workspace_root = tempfile::tempdir().unwrap();
        let factory = GitWorkspaceFactory::local(source.path())
            .with_checkout("missing/ref")
            .with_workspace_root(workspace_root.path());

        let Err(error) = factory.allocate(WorkspaceConfig::default()).await else {
            panic!("checkout unexpectedly succeeded")
        };

        assert!(error.to_string().contains("git checkout"));
        assert_eq!(fs::read_dir(workspace_root.path()).unwrap().count(), 0);
    });
}

#[test]
fn git_workspace_timeout_drains_child_output() {
    block_on(async {
        let source = workspace_fixture_repo();
        let factory = GitWorkspaceFactory::local(source.path());
        let mut workspace = factory.allocate(WorkspaceConfig::default()).await.unwrap();
        let mut slot = workspace.slot(WorkspacePath::root()).unwrap();
        let mut command = Command::new("sh");
        command.args = vec![
            "-c".to_owned(),
            "i=0; while [ $i -lt 20000 ]; do printf xxxxxxxxxx; i=$((i + 1)); done; printf done >&2"
                .to_owned(),
        ];
        command.limits.timeout = Some(Duration::from_secs(5));

        let output = slot.run_command(command).unwrap();

        assert_eq!(output.status.code, Some(0));
        assert_eq!(output.stdout.bytes.len(), 200_000);
        assert_eq!(output.stderr.bytes, b"done");
        drop(slot);
        workspace.cleanup().await.unwrap();
    });
}

#[cfg(unix)]
#[test]
fn git_workspace_lists_symlinks_without_following_them() {
    block_on(async {
        let source = workspace_fixture_repo();
        let outside = tempfile::tempdir().unwrap();
        fs::write(outside.path().join("outside.txt"), "outside\n").unwrap();
        let factory = GitWorkspaceFactory::local(source.path());
        let mut workspace = factory.allocate(WorkspaceConfig::default()).await.unwrap();
        let mount = workspace
            .local_mount()
            .expect("git workspace has local mount")
            .to_path_buf();
        std::os::unix::fs::symlink(outside.path(), mount.join("linked-dir")).unwrap();
        std::os::unix::fs::symlink("missing-target", mount.join("dangling-link")).unwrap();

        let files = workspace
            .slot(WorkspacePath::root())
            .unwrap()
            .list_files(&WorkspacePath::root())
            .unwrap();

        assert!(files.contains(&WorkspacePath::new("linked-dir").unwrap()));
        assert!(files.contains(&WorkspacePath::new("dangling-link").unwrap()));
        assert!(!files.contains(&WorkspacePath::new("linked-dir/outside.txt").unwrap()));
        workspace.cleanup().await.unwrap();
    });
}

#[test]
fn git_checkout_captures_restores_and_deletes_program_refs() {
    block_on(async {
        let source = workspace_fixture_repo();
        let factory = GitWorkspaceFactory::local(source.path());
        let workspace = factory.allocate(WorkspaceConfig::default()).await.unwrap();
        let mount = workspace
            .local_mount()
            .expect("git workspace has local mount")
            .to_path_buf();

        GitCheckout::restore_ref(&mount, &branch_key("program/base")).unwrap();
        assert_eq!(
            checked_out_ref(&mount),
            "program/base",
            "branch restore should attach HEAD to the branch"
        );
        fs::write(mount.join("program.txt"), "child program\n").unwrap();
        run_git(&mount, ["checkout", "-b", "program/child"]);
        run_git(&mount, ["add", "program.txt"]);
        run_git_with_identity(&mount, ["commit", "-m", "child program"]);
        run_git(&mount, ["tag", "frontier/child"]);

        let artifact = GitCheckout::capture(&mount).unwrap();
        assert!(artifact.ref_by_key(&branch_key("program/base")).is_some());
        assert!(artifact.ref_by_key(&branch_key("program/child")).is_some());
        assert!(artifact.ref_by_key(&tag_key("frontier/base")).is_some());
        assert!(artifact.ref_by_key(&tag_key("frontier/child")).is_some());
        assert_eq!(
            artifact
                .files()
                .get(&leaven_artifact_git::GitPath::new("program.txt").unwrap())
                .map(Vec::as_slice),
            Some(b"child program\n".as_slice())
        );

        GitCheckout::restore_ref(&mount, &branch_key("program/base")).unwrap();
        assert_eq!(
            fs::read_to_string(mount.join("program.txt")).unwrap(),
            "program branch\n"
        );

        GitCheckout::delete_ref(&mount, &branch_key("program/child")).unwrap();
        GitCheckout::delete_ref(&mount, &tag_key("frontier/child")).unwrap();
        let after_delete = GitCheckout::capture(&mount).unwrap();
        assert!(
            after_delete
                .ref_by_key(&branch_key("program/child"))
                .is_none()
        );
        assert!(
            after_delete
                .ref_by_key(&tag_key("frontier/child"))
                .is_none()
        );

        workspace.cleanup().await.unwrap();
    });
}

#[test]
fn git_checkout_restores_tag_when_branch_has_same_short_name() {
    block_on(async {
        let source = workspace_fixture_repo();
        let factory = GitWorkspaceFactory::local(source.path());
        let workspace = factory.allocate(WorkspaceConfig::default()).await.unwrap();
        let mount = workspace
            .local_mount()
            .expect("git workspace has local mount")
            .to_path_buf();

        run_git(&mount, ["checkout", "main"]);
        run_git(&mount, ["tag", "program/base"]);

        GitCheckout::restore_ref(&mount, &tag_key("program/base")).unwrap();
        assert_eq!(
            fs::read_to_string(mount.join("program.txt")).unwrap(),
            "base\n"
        );

        workspace.cleanup().await.unwrap();
    });
}

#[test]
fn git_checkout_captures_untracked_files_deletions_and_symlinks() {
    block_on(async {
        let source = workspace_fixture_repo();
        let factory = GitWorkspaceFactory::local(source.path());
        let workspace = factory.allocate(WorkspaceConfig::default()).await.unwrap();
        let mount = workspace
            .local_mount()
            .expect("git workspace has local mount")
            .to_path_buf();

        fs::remove_file(mount.join("program.txt")).unwrap();
        fs::write(mount.join("scratch.txt"), "untracked\n").unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink("scratch.txt", mount.join("scratch-link")).unwrap();

        let artifact = GitCheckout::capture(&mount).unwrap();

        assert!(
            artifact
                .files()
                .get(&leaven_artifact_git::GitPath::new("program.txt").unwrap())
                .is_none(),
            "deleted tracked files should be absent from the captured artifact"
        );
        assert_eq!(
            artifact
                .files()
                .get(&leaven_artifact_git::GitPath::new("scratch.txt").unwrap())
                .map(Vec::as_slice),
            Some(b"untracked\n".as_slice())
        );
        #[cfg(unix)]
        assert_eq!(
            artifact
                .files()
                .get(&leaven_artifact_git::GitPath::new("scratch-link").unwrap())
                .map(Vec::as_slice),
            Some(b"scratch.txt".as_slice()),
            "symlinks should capture the link payload, not the target file bytes"
        );

        workspace.cleanup().await.unwrap();
    });
}
