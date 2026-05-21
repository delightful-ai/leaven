use std::fs;
use std::path::Path;
use std::process::Command as ProcessCommand;

use leaven_artifact_git::{GitObjectId, GitRefKey, GitRefKind, GitRefName, GitRevision};
use leaven_workspace_git::{
    GitCommitImportRequest, GitCommitImporter, GitProjection, GitProjectionRequest,
    GitWorkspaceGitError,
};

#[test]
fn git_projection_contains_allowed_refs_without_hidden_objects_or_alternates() {
    let source = fixture_repo();
    let projection_root = tempfile::tempdir().unwrap();
    let projection_path = projection_root.path().join("archive.git");

    let hidden_commit = git_output(source.path(), ["rev-parse", "refs/heads/hidden/eval"]);
    let hidden_commit = hidden_commit.trim();

    let projection = GitProjection::create_bare(GitProjectionRequest {
        source: source.path().to_path_buf(),
        destination: projection_path.clone(),
        allowed_refs: vec![
            branch_key("program/base"),
            tag_key("frontier/base"),
            branch_key("program/peer"),
        ],
    })
    .unwrap();

    assert_eq!(projection.path(), projection_path.as_path());
    assert_eq!(
        git_output(&projection_path, ["rev-parse", "refs/heads/program/base"]).trim(),
        git_output(source.path(), ["rev-parse", "refs/heads/program/base"]).trim()
    );
    assert_eq!(
        git_output(&projection_path, ["rev-parse", "refs/tags/frontier/base"]).trim(),
        git_output(source.path(), ["rev-parse", "refs/tags/frontier/base"]).trim()
    );
    assert_git_fails(
        &projection_path,
        ["show-ref", "--verify", "refs/heads/hidden/eval"],
    );
    assert_git_fails(&projection_path, ["cat-file", "-e", hidden_commit]);
    assert!(
        !projection_path.join("objects/info/alternates").exists(),
        "projection must not borrow hidden objects through alternates"
    );
    run_git(&projection_path, ["fsck", "--strict"]);
}

#[test]
fn git_commit_import_fscks_source_before_writing_durable_store() {
    let source = fixture_repo();
    let durable = tempfile::tempdir().unwrap();
    run_git(durable.path(), ["init", "--bare"]);

    let parent = git_object(source.path(), "refs/heads/program/base");
    let child = git_object(source.path(), "refs/heads/program/peer");
    corrupt_loose_object(source.path(), &child);

    let error = GitCommitImporter::import_commit(GitCommitImportRequest {
        source: source.path().to_path_buf(),
        durable_store: durable.path().to_path_buf(),
        commit: child.clone(),
        expected_parent: parent,
    })
    .unwrap_err();

    assert!(matches!(error, GitWorkspaceGitError::Fsck { .. }));
    assert_git_fails(durable.path(), ["cat-file", "-e", child.as_str()]);
}

#[test]
fn git_commit_import_writes_child_revision_to_durable_store_after_validation() {
    let source = fixture_repo();
    let durable = tempfile::tempdir().unwrap();
    run_git(durable.path(), ["init", "--bare"]);

    let parent = git_object(source.path(), "refs/heads/program/base");
    let child = git_object(source.path(), "refs/heads/program/peer");

    let imported = GitCommitImporter::import_commit(GitCommitImportRequest {
        source: source.path().to_path_buf(),
        durable_store: durable.path().to_path_buf(),
        commit: child.clone(),
        expected_parent: parent,
    })
    .unwrap();

    assert_eq!(imported.revision(), &GitRevision::Commit(child.clone()));
    assert_eq!(
        git_output(durable.path(), ["cat-file", "-t", child.as_str()]).trim(),
        "commit"
    );
    run_git(durable.path(), ["fsck", "--strict"]);
}

fn fixture_repo() -> tempfile::TempDir {
    let source = tempfile::tempdir().unwrap();
    run_git(source.path(), ["init", "--initial-branch=main"]);
    run_git(source.path(), ["config", "user.name", "Leaven Test"]);
    run_git(
        source.path(),
        ["config", "user.email", "leaven@example.invalid"],
    );

    fs::write(source.path().join("program.txt"), "base\n").unwrap();
    run_git(source.path(), ["add", "program.txt"]);
    run_git(source.path(), ["commit", "-m", "base"]);

    run_git(source.path(), ["checkout", "-b", "program/base"]);
    fs::write(source.path().join("program.txt"), "program base\n").unwrap();
    run_git(source.path(), ["add", "program.txt"]);
    run_git(source.path(), ["commit", "-m", "program base"]);
    run_git(source.path(), ["tag", "frontier/base"]);

    run_git(source.path(), ["checkout", "-b", "program/peer"]);
    fs::write(source.path().join("program.txt"), "program peer\n").unwrap();
    run_git(source.path(), ["add", "program.txt"]);
    run_git(source.path(), ["commit", "-m", "program peer"]);

    run_git(source.path(), ["checkout", "program/base"]);
    run_git(source.path(), ["checkout", "-b", "hidden/eval"]);
    fs::write(source.path().join("hidden.txt"), "hidden evaluator data\n").unwrap();
    run_git(source.path(), ["add", "hidden.txt"]);
    run_git(source.path(), ["commit", "-m", "hidden eval"]);

    source
}

fn branch_key(name: &str) -> GitRefKey {
    GitRefKey::new(GitRefKind::Branch, GitRefName::new(name).unwrap())
}

fn tag_key(name: &str) -> GitRefKey {
    GitRefKey::new(GitRefKind::Tag, GitRefName::new(name).unwrap())
}

fn git_object(cwd: &Path, rev: &str) -> GitObjectId {
    GitObjectId::new(git_output(cwd, ["rev-parse", rev]).trim()).unwrap()
}

fn corrupt_loose_object(repo: &Path, object: &GitObjectId) {
    let hex = object.as_str();
    let object_path = repo.join(".git/objects").join(&hex[..2]).join(&hex[2..]);
    fs::remove_file(&object_path).unwrap();
    fs::write(object_path, b"not a git object").unwrap();
}

fn run_git<const N: usize>(cwd: &Path, args: [&str; N]) {
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

fn assert_git_fails<const N: usize>(cwd: &Path, args: [&str; N]) {
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
