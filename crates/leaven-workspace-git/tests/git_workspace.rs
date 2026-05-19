use std::fs;
use std::process::Command as ProcessCommand;

use futures::executor::block_on;
use leaven_workspace::{Command, WorkspaceConfig, WorkspaceFactory, WorkspacePath};
use leaven_workspace_git::GitWorkspaceFactory;

#[test]
fn git_workspace_factory_clones_and_checks_out_program_branch() {
    block_on(async {
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
