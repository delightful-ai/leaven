//! Laws for building a single-repo Git program from in-memory content and
//! reading an evolved revision back to the same flat file map.

use std::collections::BTreeMap;

use leaven_agentic_git::{
    GitAgenticGitError, GitProgramMaterializer, GitProgramReadback, build_program_seed,
    read_revision_files,
};
use leaven_artifact_git::{GitPath, GitProgramArtifact, GitRevision, RepoKey};
use leaven_core::Artifact;
use leaven_workspace::{WorkspaceConfig, WorkspaceFactory};
use leaven_workspace_local::LocalWorkspaceFactory;

fn repo_key() -> RepoKey {
    RepoKey::new("agent_kit").unwrap()
}

fn layout() -> GitPath {
    GitPath::new("repos/agent_kit").unwrap()
}

fn seed_files() -> BTreeMap<GitPath, Vec<u8>> {
    BTreeMap::from([
        (
            GitPath::new("manifest.toml").unwrap(),
            b"schema = \"v1\"\nsystem_prompt = \"system_prompt.md\"\nskills = \"skills\"\n"
                .to_vec(),
        ),
        (
            GitPath::new("system_prompt.md").unwrap(),
            b"You are a careful solver.".to_vec(),
        ),
        (
            GitPath::new("skills/arithmetic/SKILL.md").unwrap(),
            b"Add carefully.".to_vec(),
        ),
    ])
}

#[test]
fn seed_round_trips_flat_content_through_a_real_revision() {
    let store_root = tempfile::tempdir().unwrap();
    let files = seed_files();
    let seed = build_program_seed(repo_key(), layout(), store_root.path(), &files).unwrap();

    // The seed artifact validates and names a commit revision.
    seed.artifact().validate().unwrap();
    assert!(matches!(seed.revision(), GitRevision::Commit(_)));

    // Readback of the seed revision reproduces the exact flat content, including
    // the nested skill path.
    let read = read_revision_files(seed.stores(), seed.repo(), seed.revision()).unwrap();
    assert_eq!(
        read, files,
        "readback must reproduce the seed file map exactly"
    );
}

#[test]
fn identical_content_yields_a_deterministic_seed_revision() {
    let files = seed_files();
    let root_a = tempfile::tempdir().unwrap();
    let root_b = tempfile::tempdir().unwrap();
    let seed_a = build_program_seed(repo_key(), layout(), root_a.path(), &files).unwrap();
    let seed_b = build_program_seed(repo_key(), layout(), root_b.path(), &files).unwrap();
    assert_eq!(
        seed_a.revision(),
        seed_b.revision(),
        "identical kit content must produce an identical seed commit id across runs"
    );

    let mut changed = files;
    changed.insert(
        GitPath::new("system_prompt.md").unwrap(),
        b"You are a DIFFERENT solver.".to_vec(),
    );
    let root_c = tempfile::tempdir().unwrap();
    let seed_c = build_program_seed(repo_key(), layout(), root_c.path(), &changed).unwrap();
    assert_ne!(
        seed_a.revision(),
        seed_c.revision(),
        "different content must produce a different seed commit id"
    );
}

#[test]
fn empty_file_set_is_refused() {
    let store_root = tempfile::tempdir().unwrap();
    let error =
        build_program_seed(repo_key(), layout(), store_root.path(), &BTreeMap::new()).unwrap_err();
    assert!(matches!(error, GitAgenticGitError::EmptyProgramSeed));
}

#[test]
fn child_revision_reads_back_the_evolved_content() {
    futures::executor::block_on(async {
        let store_root = tempfile::tempdir().unwrap();
        let files = seed_files();
        let seed = build_program_seed(repo_key(), layout(), store_root.path(), &files).unwrap();

        // Materialize the seed, edit the system prompt in the checkout, read the
        // change back, and apply it to advance the artifact to a child revision.
        let mut workspace = LocalWorkspaceFactory::temp()
            .allocate(WorkspaceConfig::default())
            .await
            .unwrap();
        let change = {
            let mut view = workspace.view();
            GitProgramMaterializer::new(seed.stores().clone())
                .materialize_program(seed.artifact(), &mut view)
                .unwrap();
            view.write_file(
                &leaven_workspace::WorkspacePath::new("repos/agent_kit/system_prompt.md").unwrap(),
                b"You are an EVOLVED solver.",
            )
            .unwrap();
            GitProgramReadback::new(seed.stores().clone())
                .read_back_change(seed.artifact(), &mut view)
                .unwrap()
                .expect("an edited checkout produces a change")
        };
        workspace.cleanup().await.unwrap();

        let child_artifact = seed.artifact().apply_change(&change).unwrap();
        let child_revision = child_revision(&child_artifact, seed.repo());
        assert_ne!(&child_revision, seed.revision());

        let read = read_revision_files(seed.stores(), seed.repo(), &child_revision).unwrap();
        assert_eq!(
            read.get(&GitPath::new("system_prompt.md").unwrap())
                .map(Vec::as_slice),
            Some(b"You are an EVOLVED solver.".as_slice()),
            "the child readback must carry the evolved system prompt"
        );
        // The unchanged skill file is preserved.
        assert_eq!(
            read.get(&GitPath::new("skills/arithmetic/SKILL.md").unwrap())
                .map(Vec::as_slice),
            Some(b"Add carefully.".as_slice())
        );
    });
}

fn child_revision(artifact: &GitProgramArtifact, repo: &RepoKey) -> GitRevision {
    artifact.repo(repo).unwrap().revision().clone()
}
