#![allow(dead_code)]

use std::fs;
use std::path::Path;
use std::process::Command as ProcessCommand;

use leaven_artifact_git::{GitObjectId, GitRefKey, GitRefKind, GitRefName};

pub fn workspace_fixture_repo() -> tempfile::TempDir {
    let source = tempfile::tempdir().unwrap();
    run_git(source.path(), ["init", "--initial-branch=main"]);
    write_and_commit(source.path(), "base\n", "base program");
    run_git(source.path(), ["checkout", "-b", "program/base"]);
    write_and_commit(source.path(), "program branch\n", "program base");
    run_git(source.path(), ["tag", "frontier/base"]);
    source
}

pub fn projection_fixture_repo() -> tempfile::TempDir {
    let source = tempfile::Builder::new()
        .prefix("leaven projection source ")
        .tempdir()
        .unwrap();
    run_git(source.path(), ["init", "--initial-branch=main"]);
    run_git(source.path(), ["config", "user.name", "Leaven Test"]);
    run_git(
        source.path(),
        ["config", "user.email", "leaven@example.invalid"],
    );
    write_and_commit(source.path(), "base\n", "base");

    run_git(source.path(), ["checkout", "-b", "program/base"]);
    write_and_commit(source.path(), "program base\n", "program base");
    run_git(source.path(), ["tag", "frontier/base"]);

    run_git(source.path(), ["checkout", "-b", "program/peer"]);
    write_and_commit(source.path(), "program peer\n", "program peer");

    run_git(source.path(), ["checkout", "program/base"]);
    run_git(source.path(), ["checkout", "-b", "hidden/eval"]);
    fs::write(source.path().join("hidden.txt"), "hidden evaluator data\n").unwrap();
    run_git(source.path(), ["add", "hidden.txt"]);
    run_git(source.path(), ["commit", "-m", "hidden eval"]);

    source
}

fn write_and_commit(repo: &Path, contents: &str, message: &str) {
    fs::write(repo.join("program.txt"), contents).unwrap();
    run_git(repo, ["add", "program.txt"]);
    run_git_with_identity(repo, ["commit", "-m", message]);
}

pub fn checked_out_ref(cwd: &Path) -> String {
    git_output(cwd, ["rev-parse", "--abbrev-ref", "HEAD"])
        .trim()
        .to_owned()
}

pub fn branch_key(name: &str) -> GitRefKey {
    GitRefKey::new(GitRefKind::Branch, GitRefName::new(name).unwrap())
}

pub fn tag_key(name: &str) -> GitRefKey {
    GitRefKey::new(GitRefKind::Tag, GitRefName::new(name).unwrap())
}

pub fn git_object(cwd: &Path, rev: &str) -> GitObjectId {
    GitObjectId::new(git_output(cwd, ["rev-parse", rev]).trim()).unwrap()
}

pub fn run_git<const N: usize>(cwd: &Path, args: [&str; N]) {
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

pub fn run_git_with_identity<const N: usize>(cwd: &Path, args: [&str; N]) {
    let output = ProcessCommand::new("git")
        .args(args)
        .current_dir(cwd)
        .env("GIT_AUTHOR_NAME", "Leaven Test")
        .env("GIT_AUTHOR_EMAIL", "leaven@example.invalid")
        .env("GIT_COMMITTER_NAME", "Leaven Test")
        .env("GIT_COMMITTER_EMAIL", "leaven@example.invalid")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {} failed: {}",
        args.join(" "),
        String::from_utf8_lossy(&output.stderr)
    );
}

pub fn git_output<const N: usize>(cwd: &Path, args: [&str; N]) -> String {
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

pub fn assert_git_fails<const N: usize>(cwd: &Path, args: [&str; N]) {
    let output = ProcessCommand::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .unwrap();
    assert!(
        !output.status.success(),
        "git {} unexpectedly succeeded",
        args.join(" ")
    );
}
