use std::collections::BTreeMap;

use leaven_artifact_git::{
    GitArtifact, GitChange, GitLineage, GitObjectId, GitPath, GitRef, GitRefKind, GitRefName,
    GitRefTarget,
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

fn git_path(path: &str) -> GitPath {
    GitPath::new(path).unwrap()
}

fn oid(hex: &str) -> GitObjectId {
    GitObjectId::new(hex).unwrap()
}

fn ref_name(name: &str) -> GitRefName {
    GitRefName::new(name).unwrap()
}
