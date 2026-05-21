use std::collections::BTreeMap;

use leaven_artifact_git::{
    GitArtifactError, GitArtifactIdentityMode, GitPath, GitProgramArtifact, GitProgramChange,
    GitProgramLayout, GitRepoArtifact, GitRepoChange, GitRevision, RepoKey, RepoRef,
};
use leaven_core::{Artifact, ArtifactIdentity, CacheIdentity};

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
