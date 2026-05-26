use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;

use leaven_agentic_git::GitProgramStores;
use leaven_artifact_git::{
    GitArtifactIdentityMode, GitObjectId, GitPath, GitProgramArtifact, GitProgramLayout,
    GitRepoArtifact, GitRevision, RepoKey, RepoRef,
};
use leaven_workspace::WorkspacePath;

pub struct GitProgramFixture {
    pub store: PathBuf,
    parent: GitRevision,
    _temp: tempfile::TempDir,
}

impl GitProgramFixture {
    pub fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source");
        let store = temp.path().join("program.git");
        create_repo(&source);
        run_git_at(temp.path(), ["clone", "--bare", "source", "program.git"]);
        let parent = GitRevision::Commit(git_object(&source, "main"));
        Self {
            store,
            parent,
            _temp: temp,
        }
    }

    pub fn stores(&self) -> GitProgramStores {
        GitProgramStores::new(BTreeMap::from([(repo_key("program"), self.store.clone())])).unwrap()
    }

    pub fn artifact(&self) -> GitProgramArtifact {
        GitProgramArtifact::new(
            BTreeMap::from([(
                repo_key("program"),
                GitRepoArtifact::new(
                    RepoRef::global(repo_key("program")),
                    self.parent.clone(),
                    None,
                    GitArtifactIdentityMode::Commit,
                ),
            )]),
            GitProgramLayout::new(BTreeMap::from([(
                repo_key("program"),
                git_path("repos/program"),
            )]))
            .unwrap(),
        )
        .unwrap()
    }
}

fn create_repo(root: &Path) {
    fs::create_dir_all(root).unwrap();
    run_git(root, ["init", "--initial-branch=main"]);
    run_git(root, ["config", "user.name", "Leaven Test"]);
    run_git(root, ["config", "user.email", "leaven@example.invalid"]);
    fs::write(root.join("program.txt"), "program base\n").unwrap();
    run_git(root, ["add", "program.txt"]);
    run_git(root, ["commit", "-m", "base"]);
}

fn run_git<const N: usize>(cwd: &Path, args: [&str; N]) {
    run_git_at(cwd, args);
}

fn run_git_at<const N: usize>(cwd: &Path, args: [&str; N]) {
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

fn repo_key(key: &str) -> RepoKey {
    RepoKey::new(key).unwrap()
}

fn git_path(path: &str) -> GitPath {
    GitPath::new(path).unwrap()
}

pub fn workspace_path(path: &str) -> WorkspacePath {
    WorkspacePath::new(path).unwrap()
}

fn git_object(cwd: &Path, rev: &str) -> GitObjectId {
    GitObjectId::new(git_output(cwd, ["rev-parse", rev]).trim()).unwrap()
}
