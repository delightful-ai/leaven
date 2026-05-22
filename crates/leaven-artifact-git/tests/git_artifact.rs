use std::collections::BTreeMap;

use leaven_artifact_git::{
    GitArtifact, GitArtifactError, GitChange, GitDiff, GitDiffSummary, GitLineage, GitObjectId,
    GitPath, GitRef, GitRefKind, GitRefName, GitRefTarget,
};
use leaven_core::{Artifact, ArtifactIdentity, CacheIdentity};

#[test]
fn git_artifact_tracks_program_branch_and_frontier_tag_lineage() {
    let base_oid = oid("1111111111111111111111111111111111111111");
    let child_oid = oid("2222222222222222222222222222222222222222");

    let base_program = GitRef::new(
        GitRefKind::Branch,
        ref_name("program/base"),
        GitRefTarget::Object(base_oid),
    )
    .with_lineage(GitLineage::root());
    let child_program = GitRef::new(
        GitRefKind::Branch,
        ref_name("program/iter-skill-1"),
        GitRefTarget::Object(child_oid.clone()),
    )
    .with_lineage(GitLineage::child(base_program.key(), 1));
    let child_frontier = GitRef::new(
        GitRefKind::Tag,
        ref_name("frontier/iter-skill-1"),
        GitRefTarget::Object(child_oid),
    )
    .with_metadata("score", "0.91");

    let artifact = GitArtifact::new(BTreeMap::from([(
        git_path(".claude/program.yaml"),
        b"name: iter-skill-1\nparent: program/base\n".to_vec(),
    )]))
    .apply_change(&GitChange::Atomic(vec![
        GitChange::UpsertRef(base_program.clone()),
        GitChange::UpsertRef(child_program.clone()),
        GitChange::UpsertRef(child_frontier.clone()),
    ]))
    .unwrap();

    assert_eq!(
        artifact.ref_by_key(child_program.key()),
        Some(&child_program)
    );
    assert_eq!(
        artifact.ref_by_key(child_frontier.key()),
        Some(&child_frontier)
    );
    assert_eq!(
        artifact
            .ref_by_key(child_program.key())
            .and_then(|reference| reference.lineage())
            .and_then(GitLineage::parent),
        Some(base_program.key())
    );
    assert_eq!(
        artifact.refs_for_prefix("frontier/").collect::<Vec<_>>(),
        vec![&child_frontier]
    );
    match (artifact.identity(), artifact.cache_identity().unwrap()) {
        (ArtifactIdentity::Content(identity), CacheIdentity::Content(cache)) => {
            assert_eq!(identity, cache);
        }
        other => panic!("unexpected git artifact identity shape: {other:?}"),
    }
}

#[test]
fn git_artifact_ref_removal_models_discarded_candidates() {
    let child = GitRef::new(
        GitRefKind::Branch,
        ref_name("program/iter-skill-2"),
        GitRefTarget::Object(oid("3333333333333333333333333333333333333333")),
    )
    .with_lineage(GitLineage::child(
        GitRef::new(
            GitRefKind::Branch,
            ref_name("program/base"),
            GitRefTarget::Object(oid("1111111111111111111111111111111111111111")),
        )
        .key(),
        1,
    ));
    let frontier = GitRef::new(
        GitRefKind::Tag,
        ref_name("frontier/iter-skill-2"),
        GitRefTarget::Object(oid("3333333333333333333333333333333333333333")),
    );

    let admitted = GitArtifact::empty()
        .apply_change(&GitChange::Atomic(vec![
            GitChange::UpsertRef(child.clone()),
            GitChange::UpsertRef(frontier.clone()),
        ]))
        .unwrap();
    let discarded = admitted
        .apply_change(&GitChange::Atomic(vec![
            GitChange::RemoveRef(child.key().clone()),
            GitChange::RemoveRef(frontier.key().clone()),
        ]))
        .unwrap();

    assert!(admitted.ref_by_key(child.key()).is_some());
    assert!(admitted.ref_by_key(frontier.key()).is_some());
    assert!(discarded.ref_by_key(child.key()).is_none());
    assert!(discarded.ref_by_key(frontier.key()).is_none());
    assert_ne!(admitted.identity(), discarded.identity());
}

#[test]
fn git_artifact_rejects_non_normal_git_inputs() {
    assert!(GitPath::new("../escape").is_err());
    assert!(GitRefName::new("program//child").is_err());
    assert!(GitRefName::new("frontier/bad lock.lock").is_err());
    assert!(GitRefName::new("program/@{bad").is_err());
    assert!(GitRefName::new("program/.hidden").is_err());
    assert!(GitRefName::new("program/trailing.").is_err());
    assert!(GitRefName::new("@").is_err());
    assert!(GitObjectId::new("not-a-commit").is_err());
}

#[test]
fn git_ref_fingerprints_separate_adjacent_fields() {
    let target = GitRefTarget::Object(oid("1111111111111111111111111111111111111111"));
    let first = GitRef::new(GitRefKind::Branch, ref_name("program/a"), target.clone())
        .with_metadata("ab", "c");
    let second =
        GitRef::new(GitRefKind::Branch, ref_name("program/a"), target).with_metadata("a", "bc");

    let first_artifact = GitArtifact::empty()
        .apply_change(&GitChange::UpsertRef(first))
        .unwrap();
    let second_artifact = GitArtifact::empty()
        .apply_change(&GitChange::UpsertRef(second))
        .unwrap();

    assert_ne!(first_artifact.identity(), second_artifact.identity());
}

#[test]
fn git_ref_fingerprints_separate_symbolic_targets_from_lineage_tags() {
    let target = GitRefTarget::Symbolic(ref_name("refs/heads/program/alineage"));
    let parent = GitRef::new(
        GitRefKind::Branch,
        ref_name("program/base"),
        GitRefTarget::Object(oid("1111111111111111111111111111111111111111")),
    );
    let with_longer_target = GitRef::new(GitRefKind::Branch, ref_name("program/a"), target);
    let with_lineage = GitRef::new(
        GitRefKind::Branch,
        ref_name("program/a"),
        GitRefTarget::Symbolic(ref_name("refs/heads/program/a")),
    )
    .with_lineage(GitLineage::child(parent.key(), 1));

    let first_artifact = GitArtifact::empty()
        .apply_change(&GitChange::UpsertRef(with_longer_target))
        .unwrap();
    let second_artifact = GitArtifact::empty()
        .apply_change(&GitChange::UpsertRef(with_lineage))
        .unwrap();

    assert_ne!(first_artifact.identity(), second_artifact.identity());
}

#[test]
fn git_file_fingerprints_separate_path_and_payload_fields() {
    let first = GitArtifact::new(BTreeMap::from([(git_path("program/ab"), b"c".to_vec())]));
    let second = GitArtifact::new(BTreeMap::from([(git_path("program/a"), b"bc".to_vec())]));

    assert_ne!(first.identity(), second.identity());
}

#[test]
fn git_refs_preserve_symbolic_targets_metadata_and_display_contracts() {
    let symbolic = GitRef::new(
        GitRefKind::Branch,
        ref_name("program/current"),
        GitRefTarget::Symbolic(ref_name("program/base")),
    )
    .with_lineage(GitLineage::root())
    .with_metadata("role", "active");
    let child_lineage = GitLineage::child(symbolic.key(), 2);
    let tag = GitRef::new(
        GitRefKind::Tag,
        ref_name("frontier/current"),
        GitRefTarget::Object(oid("AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA")),
    )
    .with_lineage(child_lineage.clone());
    let artifact = GitArtifact::empty()
        .apply_change(&GitChange::Atomic(vec![
            GitChange::UpsertRef(symbolic.clone()),
            GitChange::UpsertRef(tag.clone()),
        ]))
        .unwrap();

    assert_eq!(GitRefKind::Branch.to_string(), "branch");
    assert_eq!(GitRefKind::Tag.to_string(), "tag");
    assert_eq!(symbolic.key().to_string(), "branch:program/current");
    assert_eq!(symbolic.name().as_str(), "program/current");
    assert!(matches!(
        symbolic.target(),
        GitRefTarget::Symbolic(name) if name.as_str() == "program/base"
    ));
    assert_eq!(symbolic.lineage().and_then(GitLineage::parent), None);
    assert_eq!(symbolic.lineage().unwrap().generation(), 0);
    assert_eq!(child_lineage.parent(), Some(symbolic.key()));
    assert_eq!(child_lineage.generation(), 2);
    assert_eq!(
        symbolic.metadata().get("role").map(String::as_str),
        Some("active")
    );
    assert_eq!(tag.key().kind(), GitRefKind::Tag);
    assert_eq!(tag.key().name().as_str(), "frontier/current");
    assert!(artifact.ref_by_key(symbolic.key()).is_some());
    assert_eq!(
        artifact.refs_for_prefix("program/").collect::<Vec<_>>(),
        vec![&symbolic]
    );
}

#[test]
fn git_paths_refs_and_object_ids_reject_each_non_normal_boundary() {
    assert!(GitPath::new("").is_err());
    assert!(GitPath::new("/absolute").is_err());
    assert!(GitPath::new("bad\\slash").is_err());
    assert!(GitPath::new("nul\0path").is_err());
    assert!(GitPath::new("double//slash").is_err());
    assert!(GitPath::new("./relative").is_err());
    assert!(GitPath::new("parent/..").is_err());

    assert!(GitRefName::new("").is_err());
    assert!(GitRefName::new("/program").is_err());
    assert!(GitRefName::new("program/").is_err());
    assert!(GitRefName::new("program..child").is_err());
    assert!(GitRefName::new("program/current.LOCK").is_err());
    assert!(GitRefName::new("program/~child").is_err());
    assert!(GitRefName::new("program/[child]").is_err());
    assert!(GitRefName::new("program/.").is_err());

    assert!(GitObjectId::new("a").is_err());
    assert!(GitObjectId::new("z000000000000000000000000000000000000000").is_err());
    assert_eq!(
        GitObjectId::new("AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA")
            .unwrap()
            .as_str(),
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
    );
    assert_eq!(
        GitObjectId::new("BBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBBB",)
            .unwrap()
            .as_str(),
        "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
    );
}

#[test]
fn git_artifact_file_changes_and_diff_summary_are_public_contracts() {
    let main = git_path("src/main.rs");
    let lib = git_path("src/lib.rs");
    let readme = git_path("README.md");
    let base = GitArtifact::empty();
    let written = base
        .apply_change(&GitChange::WriteFile {
            path: main.clone(),
            bytes: b"fn main() {}\n".to_vec(),
        })
        .unwrap();
    let replaced = written
        .apply_change(&GitChange::ReplaceFiles(BTreeMap::from([
            (lib.clone(), b"pub fn library() {}\n".to_vec()),
            (readme.clone(), b"# program\n".to_vec()),
        ])))
        .unwrap();
    let removed = replaced
        .apply_change(&GitChange::RemoveFile {
            path: readme.clone(),
        })
        .unwrap();
    let missing_file = removed.apply_change(&GitChange::RemoveFile {
        path: readme.clone(),
    });
    let missing_ref = removed.apply_change(&GitChange::RemoveRef(
        GitRef::new(
            GitRefKind::Branch,
            ref_name("program/missing"),
            GitRefTarget::Object(oid("1111111111111111111111111111111111111111")),
        )
        .key()
        .clone(),
    ));
    let diff = GitDiff::new(GitDiffSummary {
        files_changed: 2,
        refs_changed: 1,
    });

    assert_eq!(
        written.files().get(&main).map(Vec::as_slice),
        Some(&b"fn main() {}\n"[..])
    );
    assert!(written.refs().is_empty());
    assert_eq!(
        replaced.files().keys().collect::<Vec<_>>(),
        vec![&readme, &lib]
    );
    assert_eq!(removed.files().keys().collect::<Vec<_>>(), vec![&lib]);
    assert!(matches!(
        missing_file,
        Err(GitArtifactError::MissingPath { path }) if path == git_path("README.md")
    ));
    assert!(matches!(
        missing_ref,
        Err(GitArtifactError::MissingRef { key }) if key.name().as_str() == "program/missing"
    ));
    assert_eq!(diff.summary().files_changed, 2);
    assert_eq!(diff.summary().refs_changed, 1);
    assert_ne!(base.identity(), removed.identity());
}

fn git_path(path: &str) -> GitPath {
    GitPath::new(path).unwrap()
}

fn oid(hex: &str) -> GitObjectId {
    GitObjectId::new(hex).unwrap()
}

fn ref_name(name: &str) -> GitRefName {
    GitRefName::new(name).unwrap()
}
