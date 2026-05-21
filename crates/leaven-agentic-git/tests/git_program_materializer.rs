use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;

use futures::executor::block_on;
use leaven_agentic_git::{GitProgramMaterializer, GitProgramReadback, GitProgramStores};
use leaven_artifact_git::{
    GitArtifactIdentityMode, GitObjectId, GitPath, GitProgramArtifact, GitProgramChange,
    GitProgramLayout, GitRepoArtifact, GitRepoChange, GitRevision, RepoKey, RepoRef,
};
use leaven_core::{Evidence, OptimizationProblem};
use leaven_engine::{BudgetLedger, Materializer, RunContext, RunGraph};
use leaven_kernel::{Budget, RunId};
use leaven_workspace::{WorkspaceConfig, WorkspaceFactory, WorkspacePath};
use leaven_workspace_local::LocalWorkspaceFactory;

#[test]
fn materializer_checks_out_multiple_repos_at_artifact_revisions() {
    block_on(async {
        let fixture = GitFixture::new();
        let artifact = fixture.parent_artifact();
        let materializer = GitProgramMaterializer::new(fixture.stores());
        let workspace_root = tempfile::tempdir().unwrap();
        let mut workspace = LocalWorkspaceFactory::new(workspace_root.path())
            .allocate(WorkspaceConfig::default())
            .await
            .unwrap();
        let mut view = workspace.view();
        let (mut graph, mut budget) = graph_and_budget();
        let ctx = RunContext::<GitProblem>::new(&mut graph, &mut budget);

        let report = materializer
            .materialize_into(&artifact, &mut view, ctx.materialize_context())
            .await
            .unwrap();

        assert_eq!(report.value.files_written, 2);
        assert_eq!(
            view.read_file(&workspace_path("repos/program/program.txt"))
                .unwrap(),
            b"program base\n"
        );
        assert_eq!(
            view.read_file(&workspace_path("repos/bench/bench.txt"))
                .unwrap(),
            b"bench base\n"
        );
        assert_git_fails(
            &view
                .local_mount()
                .unwrap()
                .join(workspace_path("repos/program").to_host_relative()),
            ["show-ref", "--verify", "refs/heads/hidden/eval"],
        );

        drop(view);
        workspace.cleanup().await.unwrap();
    });
}

#[test]
fn readback_reports_no_change_for_clean_materialized_program() {
    block_on(async {
        let fixture = GitFixture::new();
        let artifact = fixture.parent_artifact();
        let mut workspace = materialized_workspace(&fixture, &artifact).await;
        let mut view = workspace.view();

        let change = GitProgramReadback::new(fixture.stores())
            .read_back_change(&artifact, &mut view)
            .unwrap();

        assert_eq!(change, None);
        drop(view);
        workspace.cleanup().await.unwrap();
    });
}

#[test]
fn readback_imports_committed_workspace_child_before_returning_change() {
    block_on(async {
        let fixture = GitFixture::new();
        let artifact = fixture.parent_artifact();
        let mut workspace = materialized_workspace(&fixture, &artifact).await;
        let mut view = workspace.view();
        workspace_git(
            &mut view,
            "repos/program",
            ["config", "user.name", "Leaven Test"],
        );
        workspace_git(
            &mut view,
            "repos/program",
            ["config", "user.email", "leaven@example.invalid"],
        );
        view.write_file(
            &workspace_path("repos/program/program.txt"),
            b"program child\n",
        )
        .unwrap();
        workspace_git(&mut view, "repos/program", ["add", "program.txt"]);
        workspace_git(
            &mut view,
            "repos/program",
            ["commit", "-m", "program child"],
        );
        let child = workspace_git_output(&mut view, "repos/program", ["rev-parse", "HEAD"]);
        let child = git_object(child.trim());

        let change = GitProgramReadback::new(fixture.stores())
            .read_back_change(&artifact, &mut view)
            .unwrap()
            .expect("committed child should produce a change");

        assert_eq!(
            change,
            GitProgramChange::AdvanceRepo {
                repo: repo_key("program"),
                expected_parent: fixture.program_parent.clone(),
                child: GitRevision::Commit(child.clone()),
            }
        );
        assert_eq!(
            git_output(&fixture.program_store, ["cat-file", "-t", child.as_str()]).trim(),
            "commit"
        );

        drop(view);
        workspace.cleanup().await.unwrap();
    });
}

#[test]
fn readback_freezes_dirty_worktree_as_imported_child_commit() {
    block_on(async {
        let fixture = GitFixture::new();
        let artifact = fixture.parent_artifact();
        let mut workspace = materialized_workspace(&fixture, &artifact).await;
        let mut view = workspace.view();
        view.write_file(
            &workspace_path("repos/program/program.txt"),
            b"dirty child\n",
        )
        .unwrap();

        let change = GitProgramReadback::new(fixture.stores())
            .read_back_change(&artifact, &mut view)
            .unwrap()
            .expect("dirty worktree should produce a change");

        let GitProgramChange::AdvanceRepo {
            repo,
            expected_parent,
            child,
        } = change
        else {
            panic!("single dirty repo should return AdvanceRepo");
        };
        assert_eq!(repo, repo_key("program"));
        assert_eq!(expected_parent, fixture.program_parent);
        let GitRevision::Commit(child) = child else {
            panic!("dirty worktree readback should freeze a commit");
        };
        assert_eq!(
            git_output(&fixture.program_store, ["cat-file", "-t", child.as_str()]).trim(),
            "commit"
        );

        drop(view);
        workspace.cleanup().await.unwrap();
    });
}

#[test]
fn readback_returns_atomic_multi_repo_change_when_multiple_repos_move() {
    block_on(async {
        let fixture = GitFixture::new();
        let artifact = fixture.parent_artifact();
        let mut workspace = materialized_workspace(&fixture, &artifact).await;
        let mut view = workspace.view();
        view.write_file(
            &workspace_path("repos/program/program.txt"),
            b"program dirty\n",
        )
        .unwrap();
        view.write_file(&workspace_path("repos/bench/bench.txt"), b"bench dirty\n")
            .unwrap();

        let change = GitProgramReadback::new(fixture.stores())
            .read_back_change(&artifact, &mut view)
            .unwrap()
            .expect("two dirty repos should produce a change");

        let GitProgramChange::AdvanceRepos { repo_changes } = change else {
            panic!("multi-repo readback should be atomic");
        };
        assert_eq!(repo_changes.len(), 2);
        assert!(matches!(
            repo_changes.get(&repo_key("program")),
            Some(GitRepoChange::AdvanceTo { expected_parent, .. })
                if expected_parent == &fixture.program_parent
        ));
        assert!(matches!(
            repo_changes.get(&repo_key("bench")),
            Some(GitRepoChange::AdvanceTo { expected_parent, .. })
                if expected_parent == &fixture.bench_parent
        ));

        drop(view);
        workspace.cleanup().await.unwrap();
    });
}

struct GitFixture {
    program_store: PathBuf,
    bench_store: PathBuf,
    program_parent: GitRevision,
    bench_parent: GitRevision,
    _temp: tempfile::TempDir,
}

impl GitFixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let program_source = temp.path().join("program-source");
        let bench_source = temp.path().join("bench-source");
        let program_store = temp.path().join("program.git");
        let bench_store = temp.path().join("bench.git");

        create_repo(&program_source, "program.txt", "program base\n");
        run_git(&program_source, ["checkout", "-b", "hidden/eval"]);
        fs::write(program_source.join("hidden.txt"), "hidden\n").unwrap();
        run_git(&program_source, ["add", "hidden.txt"]);
        run_git(&program_source, ["commit", "-m", "hidden eval"]);
        run_git(&program_source, ["checkout", "main"]);

        create_repo(&bench_source, "bench.txt", "bench base\n");
        run_git_at(
            temp.path(),
            ["clone", "--bare", "program-source", "program.git"],
        );
        run_git_at(
            temp.path(),
            ["clone", "--bare", "bench-source", "bench.git"],
        );

        let program_parent = GitRevision::Commit(git_object(
            git_output(&program_source, ["rev-parse", "main"]).trim(),
        ));
        let bench_parent = GitRevision::Commit(git_object(
            git_output(&bench_source, ["rev-parse", "main"]).trim(),
        ));

        Self {
            program_store,
            bench_store,
            program_parent,
            bench_parent,
            _temp: temp,
        }
    }

    fn stores(&self) -> GitProgramStores {
        GitProgramStores::new(BTreeMap::from([
            (repo_key("program"), self.program_store.clone()),
            (repo_key("bench"), self.bench_store.clone()),
        ]))
        .unwrap()
    }

    fn parent_artifact(&self) -> GitProgramArtifact {
        GitProgramArtifact::new(
            BTreeMap::from([
                (
                    repo_key("program"),
                    GitRepoArtifact::new(
                        RepoRef::global(repo_key("program")),
                        self.program_parent.clone(),
                        None,
                        GitArtifactIdentityMode::Commit,
                    ),
                ),
                (
                    repo_key("bench"),
                    GitRepoArtifact::new(
                        RepoRef::global(repo_key("bench")),
                        self.bench_parent.clone(),
                        None,
                        GitArtifactIdentityMode::Commit,
                    ),
                ),
            ]),
            GitProgramLayout::new(BTreeMap::from([
                (repo_key("program"), git_path("repos/program")),
                (repo_key("bench"), git_path("repos/bench")),
            ]))
            .unwrap(),
        )
        .unwrap()
    }
}

async fn materialized_workspace(
    fixture: &GitFixture,
    artifact: &GitProgramArtifact,
) -> leaven_workspace::Workspace {
    let root = tempfile::tempdir().unwrap().keep();
    let mut workspace = LocalWorkspaceFactory::new(&root)
        .allocate(WorkspaceConfig::default())
        .await
        .unwrap();
    let mut view = workspace.view();
    let (mut graph, mut budget) = graph_and_budget();
    let ctx = RunContext::<GitProblem>::new(&mut graph, &mut budget);
    GitProgramMaterializer::new(fixture.stores())
        .materialize_into(artifact, &mut view, ctx.materialize_context())
        .await
        .unwrap();
    drop(view);
    workspace
}

fn create_repo(root: &Path, file: &str, body: &str) {
    fs::create_dir_all(root).unwrap();
    run_git(root, ["init", "--initial-branch=main"]);
    run_git(root, ["config", "user.name", "Leaven Test"]);
    run_git(root, ["config", "user.email", "leaven@example.invalid"]);
    fs::write(root.join(file), body).unwrap();
    run_git(root, ["add", file]);
    run_git(root, ["commit", "-m", "base"]);
}

fn workspace_git<const N: usize>(
    view: &mut leaven_workspace::WorkspaceView<'_>,
    cwd: &str,
    args: [&str; N],
) {
    let output = workspace_git_command(view, cwd, args);
    assert!(
        output.status.code == Some(0),
        "git failed: {}",
        String::from_utf8_lossy(&output.stderr.bytes)
    );
}

fn workspace_git_output<const N: usize>(
    view: &mut leaven_workspace::WorkspaceView<'_>,
    cwd: &str,
    args: [&str; N],
) -> String {
    let output = workspace_git_command(view, cwd, args);
    assert!(
        output.status.code == Some(0),
        "git failed: {}",
        String::from_utf8_lossy(&output.stderr.bytes)
    );
    String::from_utf8(output.stdout.bytes).unwrap()
}

fn workspace_git_command<const N: usize>(
    view: &mut leaven_workspace::WorkspaceView<'_>,
    cwd: &str,
    args: [&str; N],
) -> leaven_workspace::CommandOutput {
    let mut command = leaven_workspace::Command::new("git");
    command.cwd = Some(workspace_path(cwd));
    command.args = args.into_iter().map(str::to_owned).collect();
    view.run_command(command).unwrap()
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

fn graph_and_budget() -> (RunGraph<GitProblem>, BudgetLedger) {
    (
        RunGraph::new(RunId::new()),
        BudgetLedger::new(Budget::unlimited()),
    )
}

struct GitProblem;

impl OptimizationProblem for GitProblem {
    type Artifact = GitProgramArtifact;
    type Case = ();
    type Evidence = GitEvidence;
    type ProposalAnnotations = ();
}

#[derive(Clone, Debug, PartialEq)]
struct GitEvidence;

impl Evidence for GitEvidence {}

fn repo_key(key: &str) -> RepoKey {
    RepoKey::new(key).unwrap()
}

fn git_path(path: &str) -> GitPath {
    GitPath::new(path).unwrap()
}

fn workspace_path(path: &str) -> WorkspacePath {
    WorkspacePath::new(path).unwrap()
}

fn git_object(hex: &str) -> GitObjectId {
    GitObjectId::new(hex).unwrap()
}
