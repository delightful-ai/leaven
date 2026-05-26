use std::fs;
use std::path::Path;

use leaven_artifact_git::{GitObjectId, GitRevision};
use leaven_workspace_git::{
    GitCommitImportRequest, GitCommitImporter, GitProjection, GitProjectionRequest,
    GitWorkspaceGitError,
};

#[path = "support/git.rs"]
mod git_support;

use git_support::{
    assert_git_fails, branch_key, git_object, git_output, projection_fixture_repo, run_git, tag_key,
};

#[test]
fn git_projection_contains_allowed_refs_without_hidden_objects_or_alternates() {
    let source = projection_fixture_repo();
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
    let source = projection_fixture_repo();
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
    let source = projection_fixture_repo();
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

#[test]
fn git_commit_import_does_not_promote_source_scratch_or_trusted_refs() {
    let source = projection_fixture_repo();
    let durable = tempfile::tempdir().unwrap();
    run_git(durable.path(), ["init", "--bare"]);

    let parent = git_object(source.path(), "refs/heads/program/base");
    let child = git_object(source.path(), "refs/heads/program/peer");
    run_git(
        source.path(),
        ["update-ref", "refs/frontier/forged", child.as_str()],
    );
    run_git(
        source.path(),
        ["update-ref", "refs/leaven/scratch/proposer", child.as_str()],
    );

    let imported = GitCommitImporter::import_commit(GitCommitImportRequest {
        source: source.path().to_path_buf(),
        durable_store: durable.path().to_path_buf(),
        commit: child.clone(),
        expected_parent: parent,
    })
    .unwrap();

    assert_eq!(imported.revision(), &GitRevision::Commit(child.clone()));
    assert_eq!(
        git_output(
            durable.path(),
            ["rev-parse", &format!("refs/leaven/imported/{child}")],
        )
        .trim(),
        child.as_str()
    );
    assert_git_fails(
        durable.path(),
        ["show-ref", "--verify", "refs/frontier/forged"],
    );
    assert_git_fails(
        durable.path(),
        ["show-ref", "--verify", "refs/leaven/scratch/proposer"],
    );
}

fn corrupt_loose_object(repo: &Path, object: &GitObjectId) {
    let hex = object.as_str();
    let object_path = repo.join(".git/objects").join(&hex[..2]).join(&hex[2..]);
    fs::remove_file(&object_path).unwrap();
    fs::write(object_path, b"not a git object").unwrap();
}
