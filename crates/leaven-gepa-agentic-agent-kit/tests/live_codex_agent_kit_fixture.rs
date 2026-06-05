#![allow(
    dead_code,
    reason = "fixture module is compiled standalone by all-targets clippy"
)]

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use leaven_agentic_git::GitProgramStores;
use leaven_artifact_git::{
    GitArtifactIdentityMode, GitObjectId, GitPath, GitProgramArtifact, GitProgramLayout,
    GitRepoArtifact, GitRevision, RepoKey, RepoRef,
};
use leaven_core::{Evidence, OptimizationProblem};

#[derive(Clone, Debug)]
pub struct LiveAgentKitProblem;

impl OptimizationProblem for LiveAgentKitProblem {
    type Artifact = GitProgramArtifact;
    type Case = LiveAgentKitCase;
    type Evidence = LiveAgentKitEvidence;
    type ProposalAnnotations = ();
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct LiveAgentKitCase;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct LiveAgentKitEvidence;

impl Evidence for LiveAgentKitEvidence {}

pub struct LiveAgentKitRepoFixture {
    _temp: tempfile::TempDir,
    bare_store: PathBuf,
    parent_commit: GitObjectId,
}

impl LiveAgentKitRepoFixture {
    pub(crate) fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("agent-source");
        let bare_store = temp.path().join("agent.git");
        fs::create_dir_all(&source).unwrap();
        run_host_git(&source, ["init", "--initial-branch=main"]);
        run_host_git(&source, ["config", "user.name", "Leaven Test"]);
        run_host_git(&source, ["config", "user.email", "leaven@example.invalid"]);
        write_live_agent_kit_repo(&source);
        run_host_git(&source, ["add", "."]);
        run_host_git(&source, ["commit", "-m", "seed live agent kit"]);
        let parent_commit = git_object(host_git_output(&source, ["rev-parse", "HEAD"]).trim());
        run_host_git_at(
            temp.path(),
            [
                "clone",
                "--bare",
                source.file_name().unwrap().to_str().unwrap(),
                "agent.git",
            ],
        );
        Self {
            _temp: temp,
            bare_store,
            parent_commit,
        }
    }

    pub(crate) fn stores(&self) -> GitProgramStores {
        GitProgramStores::new(BTreeMap::from([(
            repo_key("agent"),
            self.bare_store.clone(),
        )]))
        .unwrap()
    }

    pub(crate) fn program_artifact(&self) -> GitProgramArtifact {
        program_artifact(GitRevision::Commit(self.parent_commit.clone()))
    }
}

fn write_live_agent_kit_repo(root: &Path) {
    fs::create_dir_all(root.join("skills/alpha")).unwrap();
    fs::create_dir_all(root.join("hooks")).unwrap();
    fs::write(
        root.join("manifest.toml"),
        r#"
schema = "v1"
system_prompt = "system_prompt.md"
agent_docs = "AGENTS.md"
skills = "skills/"
hooks = "hooks/"
"#,
    )
    .unwrap();
    fs::write(
        root.join("system_prompt.md"),
        "Parent system prompt. The proposal stage must replace this file.\n",
    )
    .unwrap();
    fs::write(
        root.join("AGENTS.md"),
        "Do not edit files outside repos/agent during the live proposal stage.\n",
    )
    .unwrap();
    fs::write(
        root.join("skills/alpha/SKILL.md"),
        "---\nname: alpha\ndescription: Parent alpha skill.\n---\n\nParent alpha behavior.\n",
    )
    .unwrap();
    fs::write(root.join("hooks/pre-run.sh"), "exit 1\n").unwrap();
}

fn program_artifact(revision: GitRevision) -> GitProgramArtifact {
    let key = repo_key("agent");
    GitProgramArtifact::new(
        BTreeMap::from([(key.clone(), repo_artifact(key.clone(), revision))]),
        GitProgramLayout::new(BTreeMap::from([(key, git_path("repos/agent"))])).unwrap(),
    )
    .unwrap()
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

fn git_object(hex: &str) -> GitObjectId {
    GitObjectId::new(hex).unwrap()
}

fn run_host_git<const N: usize>(cwd: &Path, args: [&str; N]) {
    run_host_git_at(cwd, args);
}

fn run_host_git_at<const N: usize>(cwd: &Path, args: [&str; N]) {
    let output = std::process::Command::new("git")
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

fn host_git_output<const N: usize>(cwd: &Path, args: [&str; N]) -> String {
    let output = std::process::Command::new("git")
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
