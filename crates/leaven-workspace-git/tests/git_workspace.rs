use std::fs;
use std::process::Command as ProcessCommand;

use futures::executor::block_on;
use leaven_artifact_git::{GitRefKey, GitRefKind, GitRefName};
use leaven_workspace::{Command, WorkspaceConfig, WorkspaceFactory, WorkspacePath};
use leaven_workspace_git::{GitCheckout, GitWorkspaceFactory};

#[test]
fn git_workspace_factory_clones_and_checks_out_program_branch() {
    block_on(async {
        let source = fixture_repo();
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
fn git_checkout_captures_restores_and_deletes_program_refs() {
    block_on(async {
        let source = fixture_repo();
        let factory = GitWorkspaceFactory::local(source.path());
        let workspace = factory.allocate(WorkspaceConfig::default()).await.unwrap();
        let mount = workspace
            .local_mount()
            .expect("git workspace has local mount")
            .to_path_buf();

        GitCheckout::restore_ref(&mount, &branch_key("program/base")).unwrap();
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
        let source = fixture_repo();
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

fn fixture_repo() -> tempfile::TempDir {
    let source = tempfile::tempdir().unwrap();
    run_git(source.path(), ["init", "--initial-branch=main"]);
    fs::write(source.path().join("program.txt"), "base\n").unwrap();
    run_git(source.path(), ["add", "program.txt"]);
    run_git_with_identity(source.path(), ["commit", "-m", "base program"]);
    run_git(source.path(), ["checkout", "-b", "program/base"]);
    fs::write(source.path().join("program.txt"), "program branch\n").unwrap();
    run_git(source.path(), ["add", "program.txt"]);
    run_git_with_identity(source.path(), ["commit", "-m", "program base"]);
    run_git(source.path(), ["tag", "frontier/base"]);
    source
}

fn branch_key(name: &str) -> GitRefKey {
    GitRefKey::new(GitRefKind::Branch, GitRefName::new(name).unwrap())
}

fn tag_key(name: &str) -> GitRefKey {
    GitRefKey::new(GitRefKind::Tag, GitRefName::new(name).unwrap())
}

fn run_git<const N: usize>(cwd: &std::path::Path, args: [&str; N]) {
    let status = ProcessCommand::new("git")
        .args(args)
        .current_dir(cwd)
        .status()
        .unwrap();
    assert!(status.success());
}

fn run_git_with_identity<const N: usize>(cwd: &std::path::Path, args: [&str; N]) {
    let status = ProcessCommand::new("git")
        .args(args)
        .current_dir(cwd)
        .env("GIT_AUTHOR_NAME", "Leaven Test")
        .env("GIT_AUTHOR_EMAIL", "leaven@example.invalid")
        .env("GIT_COMMITTER_NAME", "Leaven Test")
        .env("GIT_COMMITTER_EMAIL", "leaven@example.invalid")
        .status()
        .unwrap();
    assert!(status.success());
}
