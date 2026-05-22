use std::collections::BTreeMap;

use leaven_artifact_git::{
    GitArtifactError, GitArtifactIdentityMode, GitPath, GitProgramArtifact, GitProgramChange,
    GitProgramLayout, GitRepoArtifact, GitRepoChange, GitRevision, GitRevisionKind, RemoteRef,
    RepoKey, RepoRef, RepoStoreRef,
};
use leaven_core::{Artifact, ArtifactIdentity, CacheIdentity};
use leaven_kernel::RunId;

#[test]
fn git_program_artifact_supports_single_and_multi_repo_identity() {
    let program = repo_key("program");
    let bench = repo_key("bench");

    let single = GitProgramArtifact::new(
        BTreeMap::from([(
            program.clone(),
            repo_artifact(program.clone(), commit("11")),
        )]),
        GitProgramLayout::new(BTreeMap::from([(
            program.clone(),
            git_path("repos/program"),
        )]))
        .unwrap(),
    )
    .unwrap();
    let multi = GitProgramArtifact::new(
        BTreeMap::from([
            (
                program.clone(),
                repo_artifact(program.clone(), commit("11")),
            ),
            (bench.clone(), repo_artifact(bench.clone(), tree("22"))),
        ]),
        GitProgramLayout::new(BTreeMap::from([
            (program.clone(), git_path("repos/program")),
            (bench, git_path("repos/bench")),
        ]))
        .unwrap(),
    )
    .unwrap();
    let same_program_different_layout = GitProgramArtifact::new(
        BTreeMap::from([(
            program.clone(),
            repo_artifact(program.clone(), commit("11")),
        )]),
        GitProgramLayout::new(BTreeMap::from([(program, git_path("src/program"))])).unwrap(),
    )
    .unwrap();

    assert_eq!(single.repos().len(), 1);
    assert_eq!(multi.repos().len(), 2);
    assert_ne!(single.identity(), multi.identity());
    assert_ne!(single.identity(), same_program_different_layout.identity());
    assert_eq!(
        single.layout().path_for(&repo_key("program")),
        Some(&git_path("repos/program"))
    );
    match (single.identity(), single.cache_identity().unwrap()) {
        (ArtifactIdentity::Content(identity), CacheIdentity::Content(cache)) => {
            assert_eq!(identity, cache);
        }
        other => panic!("unexpected git program identity shape: {other:?}"),
    }
}

#[test]
fn git_program_change_advances_one_repo_without_touching_others() {
    let program = repo_key("program");
    let bench = repo_key("bench");
    let parent = program_artifact(&[
        (&program, commit("11"), "repos/program"),
        (&bench, tree("22"), "repos/bench"),
    ]);

    let child = parent
        .apply_change(&GitProgramChange::AdvanceRepo {
            repo: program.clone(),
            expected_parent: commit("11"),
            child: commit("33"),
        })
        .unwrap();

    assert_eq!(child.repo(&program).unwrap().revision(), &commit("33"));
    assert_eq!(child.repo(&bench).unwrap().revision(), &tree("22"));
}

#[test]
fn git_program_change_advances_multiple_repos_atomically() {
    let program = repo_key("program");
    let bench = repo_key("bench");
    let parent = program_artifact(&[
        (&program, commit("11"), "repos/program"),
        (&bench, tree("22"), "repos/bench"),
    ]);

    let child = parent
        .apply_change(&GitProgramChange::AdvanceRepos {
            repo_changes: BTreeMap::from([
                (
                    program.clone(),
                    GitRepoChange::AdvanceTo {
                        expected_parent: commit("11"),
                        child: commit("33"),
                    },
                ),
                (
                    bench.clone(),
                    GitRepoChange::AdvanceTo {
                        expected_parent: tree("22"),
                        child: tree("44"),
                    },
                ),
            ]),
        })
        .unwrap();
    let rejected = parent.apply_change(&GitProgramChange::AdvanceRepos {
        repo_changes: BTreeMap::from([
            (
                program.clone(),
                GitRepoChange::AdvanceTo {
                    expected_parent: commit("99"),
                    child: commit("33"),
                },
            ),
            (
                bench.clone(),
                GitRepoChange::AdvanceTo {
                    expected_parent: tree("22"),
                    child: tree("44"),
                },
            ),
        ]),
    });

    assert_eq!(child.repo(&program).unwrap().revision(), &commit("33"));
    assert_eq!(child.repo(&bench).unwrap().revision(), &tree("44"));
    assert!(matches!(
        rejected,
        Err(GitArtifactError::RevisionParentMismatch { .. })
    ));
    assert_eq!(parent.repo(&program).unwrap().revision(), &commit("11"));
    assert_eq!(parent.repo(&bench).unwrap().revision(), &tree("22"));
}

#[test]
fn git_program_artifact_rejects_invalid_repo_keys_and_layout() {
    assert!(RepoKey::new("").is_err());
    assert!(RepoKey::new("../escape").is_err());
    assert!(RepoKey::new("with/slash").is_err());

    let program = repo_key("program");
    let bench = repo_key("bench");
    let missing_layout = GitProgramArtifact::new(
        BTreeMap::from([(
            program.clone(),
            repo_artifact(program.clone(), commit("11")),
        )]),
        GitProgramLayout::new(BTreeMap::new()).unwrap(),
    );
    let unknown_layout = GitProgramArtifact::new(
        BTreeMap::from([(program.clone(), repo_artifact(program, commit("11")))]),
        GitProgramLayout::new(BTreeMap::from([(bench, git_path("repos/bench"))])).unwrap(),
    );

    assert!(matches!(
        missing_layout,
        Err(GitArtifactError::MissingRepoLayout { .. })
    ));
    assert!(matches!(
        unknown_layout,
        Err(
            GitArtifactError::MissingRepoLayout { .. } | GitArtifactError::UnknownRepoLayout { .. },
        )
    ));
}

#[test]
fn repo_refs_revisions_and_subpaths_are_part_of_program_identity() {
    let program = repo_key("program");
    let bench = repo_key("bench");
    let remote = RemoteRef::new("origin/main").unwrap();
    let run_ref = RepoRef::run_scoped(RunId::new(), program.clone());
    let remote_ref = RepoRef::remote(remote.clone(), bench.clone());
    let commit_revision = commit("aa");
    let tree_revision = tree("bb");

    let without_subpath = GitProgramArtifact::new(
        BTreeMap::from([
            (
                program.clone(),
                GitRepoArtifact::new(
                    run_ref.clone(),
                    commit_revision.clone(),
                    None,
                    GitArtifactIdentityMode::Commit,
                ),
            ),
            (
                bench.clone(),
                GitRepoArtifact::new(
                    remote_ref.clone(),
                    tree_revision.clone(),
                    None,
                    GitArtifactIdentityMode::Tree,
                ),
            ),
        ]),
        GitProgramLayout::new(BTreeMap::from([
            (program.clone(), git_path("repos/program")),
            (bench.clone(), git_path("repos/bench")),
        ]))
        .unwrap(),
    )
    .unwrap();
    let with_subpath = GitProgramArtifact::new(
        BTreeMap::from([
            (
                program.clone(),
                GitRepoArtifact::new(
                    run_ref.clone(),
                    commit_revision.clone(),
                    Some(git_path("src/lib.rs")),
                    GitArtifactIdentityMode::Commit,
                ),
            ),
            (
                bench.clone(),
                GitRepoArtifact::new(
                    remote_ref.clone(),
                    tree_revision.clone(),
                    None,
                    GitArtifactIdentityMode::Tree,
                ),
            ),
        ]),
        GitProgramLayout::new(BTreeMap::from([
            (program.clone(), git_path("repos/program")),
            (bench.clone(), git_path("repos/bench")),
        ]))
        .unwrap(),
    )
    .unwrap();

    assert_eq!(commit_revision.kind(), GitRevisionKind::Commit);
    assert_eq!(tree_revision.kind(), GitRevisionKind::Tree);
    assert_eq!(
        commit_revision.to_string(),
        format!("commit:{}", "aa00000000000000000000000000000000000000")
    );
    assert_eq!(
        tree_revision.to_string(),
        format!("tree:{}", "bb00000000000000000000000000000000000000")
    );
    assert_eq!(remote.as_str(), "origin/main");
    assert_eq!(remote_ref.key(), &bench);
    assert_eq!(remote_ref.store().repo_key(), &bench);
    assert!(matches!(
        remote_ref.store(),
        RepoStoreRef::Remote { remote, repo_key } if remote.as_str() == "origin/main" && repo_key == &bench
    ));
    assert!(matches!(
        run_ref.store(),
        RepoStoreRef::RunScoped { repo_key, .. } if repo_key == &program
    ));
    assert_eq!(
        with_subpath.repo(&program).unwrap().subpath(),
        Some(&git_path("src/lib.rs"))
    );
    assert_eq!(
        with_subpath.repo(&bench).unwrap().identity_mode(),
        GitArtifactIdentityMode::Tree
    );
    assert_ne!(without_subpath.identity(), with_subpath.identity());
}

#[test]
fn git_program_artifact_rejects_invalid_boundary_cases_without_partial_apply() {
    assert!(RepoKey::new(".").is_err());
    assert!(RepoKey::new("..").is_err());
    assert!(RepoKey::new(".hidden").is_err());
    assert!(RepoKey::new("hidden.").is_err());
    assert!(RepoKey::new("with\\slash").is_err());
    assert!(RepoKey::new("bad key").is_err());
    assert!(RepoKey::new("nul\0key").is_err());
    assert!(RemoteRef::new("").is_err());
    assert!(RemoteRef::new("origin\0main").is_err());

    let program = repo_key("program");
    let bench = repo_key("bench");
    let empty = GitProgramArtifact::new(
        BTreeMap::new(),
        GitProgramLayout::new(BTreeMap::new()).unwrap(),
    );
    let mismatched_repo_key = GitProgramArtifact::new(
        BTreeMap::from([(program.clone(), repo_artifact(bench.clone(), commit("11")))]),
        GitProgramLayout::new(BTreeMap::from([(
            program.clone(),
            git_path("repos/program"),
        )]))
        .unwrap(),
    );
    let parent = program_artifact(&[(&program, commit("11"), "repos/program")]);
    let missing_repo_change = parent.apply_change(&GitProgramChange::AdvanceRepo {
        repo: bench.clone(),
        expected_parent: commit("11"),
        child: commit("33"),
    });

    assert!(matches!(empty, Err(GitArtifactError::EmptyProgram)));
    assert!(matches!(
        mismatched_repo_key,
        Err(GitArtifactError::RepoKeyMismatch { .. })
    ));
    assert!(matches!(
        missing_repo_change,
        Err(GitArtifactError::MissingRepo { repo }) if repo == bench
    ));
    assert_eq!(parent.repo(&program).unwrap().revision(), &commit("11"));
}

fn program_artifact(specs: &[(&RepoKey, GitRevision, &str)]) -> GitProgramArtifact {
    let mut repos = BTreeMap::new();
    let mut layout = BTreeMap::new();
    for (key, revision, path) in specs {
        repos.insert(
            (*key).clone(),
            repo_artifact((*key).clone(), revision.clone()),
        );
        layout.insert((*key).clone(), git_path(path));
    }

    GitProgramArtifact::new(repos, GitProgramLayout::new(layout).unwrap()).unwrap()
}

fn repo_artifact(key: RepoKey, revision: GitRevision) -> GitRepoArtifact {
    GitRepoArtifact::new(
        RepoRef::global(key),
        revision,
        None,
        GitArtifactIdentityMode::Commit,
    )
}

fn repo_key(value: &str) -> RepoKey {
    RepoKey::new(value).unwrap()
}

fn git_path(path: &str) -> GitPath {
    GitPath::new(path).unwrap()
}

fn commit(byte: &str) -> GitRevision {
    GitRevision::commit(format!("{byte:0<40}")).unwrap()
}

fn tree(byte: &str) -> GitRevision {
    GitRevision::tree(format!("{byte:0<40}")).unwrap()
}
