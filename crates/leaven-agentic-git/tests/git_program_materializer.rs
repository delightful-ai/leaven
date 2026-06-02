use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command as ProcessCommand;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Instant;

use futures::executor::block_on;
use futures::future::{BoxFuture, FutureExt};
use leaven_agentic_git::{
    GitAgenticGitError, GitProgramMaterializer, GitProgramReadback, GitProgramStores,
};
use leaven_artifact_git::{
    GitArtifactIdentityMode, GitObjectId, GitPath, GitProgramArtifact, GitProgramChange,
    GitProgramLayout, GitRepoArtifact, GitRepoChange, GitRevision, RepoKey, RepoRef,
};
use leaven_core::{Artifact, Evidence, OptimizationProblem};
use leaven_engine::{BudgetLedger, Materializer, RunContext, RunGraph};
use leaven_kernel::{Budget, RunId};
use leaven_workspace::{
    CapturedOutput, Command as WorkspaceCommand, CommandStdin, ExitStatus, FactoryError, Workspace,
    WorkspaceBackend, WorkspaceConfig, WorkspaceError, WorkspaceFactory, WorkspacePath,
};
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
fn materializer_rejects_tree_revisions_as_outside_commit_adapter_contract() {
    let fixture = GitFixture::new();
    let artifact = fixture.program_tree_artifact();
    let materializer = GitProgramMaterializer::new(fixture.stores());
    let workspace_root = tempfile::tempdir().unwrap();
    let mut workspace = block_on(
        LocalWorkspaceFactory::new(workspace_root.path()).allocate(WorkspaceConfig::default()),
    )
    .unwrap();
    let mut view = workspace.view();

    let error = materializer
        .materialize_program(&artifact, &mut view)
        .expect_err("tree revision should not enter commit materializer");

    assert!(matches!(
        error,
        GitAgenticGitError::NonCommitMaterialization { repo } if repo == repo_key("program")
    ));
    drop(view);
    block_on(workspace.cleanup()).unwrap();
}

#[test]
fn materializer_does_not_expose_host_store_paths_to_workspace_commands() {
    block_on(async {
        let fixture = GitFixture::new();
        let artifact = fixture.program_artifact();
        let materializer = GitProgramMaterializer::new(fixture.stores());
        let workspace_root = tempfile::tempdir().unwrap().keep();
        let mut workspace = NoLocalMountWorkspaceFactory::new(workspace_root)
            .rejecting_command_fragments(vec![fixture.program_store.display().to_string()])
            .allocate(WorkspaceConfig::default())
            .await
            .unwrap();
        assert!(workspace.local_mount().is_none());
        let mut view = workspace.view();
        let (mut graph, mut budget) = graph_and_budget();
        let ctx = RunContext::<GitProblem>::new(&mut graph, &mut budget);

        materializer
            .materialize_into(&artifact, &mut view, ctx.materialize_context())
            .await
            .unwrap();

        let config = String::from_utf8(
            view.read_file(&workspace_path("repos/program/.git/config"))
                .unwrap(),
        )
        .unwrap();
        assert!(
            !config.contains(&fixture.program_store.display().to_string()),
            "workspace git config leaked host durable store path: {config}"
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
fn readback_rejects_tree_revisions_as_outside_commit_adapter_contract() {
    let fixture = GitFixture::new();
    let artifact = fixture.program_tree_artifact();
    let root = tempfile::tempdir().unwrap();
    let mut workspace =
        block_on(LocalWorkspaceFactory::new(root.path()).allocate(WorkspaceConfig::default()))
            .unwrap();
    let mut view = workspace.view();

    let error = GitProgramReadback::new(fixture.stores())
        .read_back_change(&artifact, &mut view)
        .expect_err("tree revision should not enter commit readback");

    assert!(matches!(
        error,
        GitAgenticGitError::NonCommitReadback { repo } if repo == repo_key("program")
    ));
    drop(view);
    block_on(workspace.cleanup()).unwrap();
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
fn readback_imports_output_bundle_proposal_before_checkout_state() {
    block_on(async {
        let fixture = GitFixture::new();
        let artifact = fixture.program_artifact();
        let mut workspace = materialized_workspace(&fixture, &artifact).await;
        let mut view = workspace.view();
        configure_workspace_git(&mut view, "repos/program");
        view.write_file(
            &workspace_path("repos/program/program.txt"),
            b"bundled child\n",
        )
        .unwrap();
        workspace_git(&mut view, "repos/program", ["add", "program.txt"]);
        workspace_git(
            &mut view,
            "repos/program",
            ["commit", "-m", "bundled child"],
        );
        let child = workspace_git_output(&mut view, "repos/program", ["rev-parse", "HEAD"]);
        let child = git_object(child.trim());
        workspace_command(&mut view, None, "mkdir", ["-p", "output"]);
        let child_range = format!("{}..HEAD", fixture.program_parent.object_id().as_str());
        workspace_git(
            &mut view,
            "repos/program",
            [
                "bundle",
                "create",
                "../../output/proposal.bundle",
                child_range.as_str(),
            ],
        );
        workspace_git(
            &mut view,
            "repos/program",
            [
                "reset",
                "--hard",
                fixture.program_parent.object_id().as_str(),
            ],
        );

        let change = GitProgramReadback::new(fixture.stores())
            .read_back_change(&artifact, &mut view)
            .unwrap()
            .expect("output bundle should produce a change");

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
fn readback_imports_output_patch_proposal_as_child_commit() {
    block_on(async {
        let fixture = GitFixture::new();
        let artifact = fixture.program_artifact();
        let mut workspace = materialized_workspace(&fixture, &artifact).await;
        let mut view = workspace.view();
        view.write_file(
            &workspace_path("repos/program/program.txt"),
            b"patched child\n",
        )
        .unwrap();
        let patch = workspace_git_output(&mut view, "repos/program", ["diff", "--binary"]);
        view.write_file(&workspace_path("output/proposal.patch"), patch.as_bytes())
            .unwrap();
        workspace_git(
            &mut view,
            "repos/program",
            ["checkout", "--", "program.txt"],
        );

        let change = GitProgramReadback::new(fixture.stores())
            .read_back_change(&artifact, &mut view)
            .unwrap()
            .expect("output patch should produce a change");

        let GitProgramChange::AdvanceRepo {
            repo,
            expected_parent,
            child,
        } = change
        else {
            panic!("single output patch should return AdvanceRepo");
        };
        assert_eq!(repo, repo_key("program"));
        assert_eq!(expected_parent, fixture.program_parent);
        let GitRevision::Commit(child) = child else {
            panic!("patch readback should create a commit");
        };
        assert_eq!(
            git_output(
                &fixture.program_store,
                ["show", &format!("{child}:program.txt")],
            ),
            "patched child\n"
        );

        drop(view);
        workspace.cleanup().await.unwrap();
    });
}

#[test]
fn readback_imports_output_patch_without_local_mount() {
    block_on(async {
        let fixture = GitFixture::new();
        let artifact = fixture.program_artifact();
        let mut workspace = materialized_workspace_without_local_mount(&fixture, &artifact).await;
        assert!(workspace.local_mount().is_none());
        let mut view = workspace.view();
        view.write_file(
            &workspace_path("repos/program/program.txt"),
            b"patched child without mount\n",
        )
        .unwrap();
        let patch = workspace_git_output(&mut view, "repos/program", ["diff", "--binary"]);
        view.write_file(&workspace_path("output/proposal.patch"), patch.as_bytes())
            .unwrap();
        workspace_git(
            &mut view,
            "repos/program",
            ["checkout", "--", "program.txt"],
        );

        let change = GitProgramReadback::new(fixture.stores())
            .read_back_change(&artifact, &mut view)
            .unwrap()
            .expect("output patch should produce a change without local_mount");

        let GitProgramChange::AdvanceRepo { child, .. } = change else {
            panic!("single output patch should return AdvanceRepo");
        };
        let GitRevision::Commit(child) = child else {
            panic!("patch readback should create a commit");
        };
        assert_eq!(
            git_output(
                &fixture.program_store,
                ["show", &format!("{child}:program.txt")],
            ),
            "patched child without mount\n"
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
fn readback_freezes_dirty_worktree_without_local_mount() {
    block_on(async {
        let fixture = GitFixture::new();
        let artifact = fixture.parent_artifact();
        let mut workspace = materialized_workspace_without_local_mount(&fixture, &artifact).await;
        assert!(workspace.local_mount().is_none());
        let mut view = workspace.view();
        view.write_file(
            &workspace_path("repos/program/program.txt"),
            b"dirty child without mount\n",
        )
        .unwrap();

        let change = GitProgramReadback::new(fixture.stores())
            .read_back_change(&artifact, &mut view)
            .unwrap()
            .expect("dirty worktree should produce a change without local_mount");

        let GitProgramChange::AdvanceRepo { child, .. } = change else {
            panic!("single dirty repo should return AdvanceRepo");
        };
        let GitRevision::Commit(child) = child else {
            panic!("dirty worktree readback should freeze a commit");
        };
        assert_eq!(
            git_output(
                &fixture.program_store,
                ["show", &format!("{child}:program.txt")],
            ),
            "dirty child without mount\n"
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

#[test]
fn readback_children_rematerialize_every_multi_repo_intermediate() {
    block_on(async {
        let fixture = GitFixture::new();
        let mut artifact = fixture.parent_artifact();

        for step in 1..=2 {
            let mut workspace = materialized_workspace(&fixture, &artifact).await;
            let mut view = workspace.view();
            let program_body = format!("program intermediate {step}\n");
            let bench_body = format!("bench intermediate {step}\n");
            view.write_file(
                &workspace_path("repos/program/program.txt"),
                program_body.as_bytes(),
            )
            .unwrap();
            view.write_file(
                &workspace_path("repos/bench/bench.txt"),
                bench_body.as_bytes(),
            )
            .unwrap();

            let change = GitProgramReadback::new(fixture.stores())
                .read_back_change(&artifact, &mut view)
                .unwrap()
                .expect("dirty multi-repo intermediate should produce a change");
            let GitProgramChange::AdvanceRepos { repo_changes } = &change else {
                panic!("multi-repo intermediate should be imported atomically");
            };
            assert_eq!(repo_changes.len(), 2);
            assert!(matches!(
                repo_changes.get(&repo_key("program")),
                Some(GitRepoChange::AdvanceTo {
                    expected_parent,
                    child
                }) if expected_parent == artifact.repo(&repo_key("program")).unwrap().revision()
                    && child != expected_parent
            ));
            assert!(matches!(
                repo_changes.get(&repo_key("bench")),
                Some(GitRepoChange::AdvanceTo {
                    expected_parent,
                    child
                }) if expected_parent == artifact.repo(&repo_key("bench")).unwrap().revision()
                    && child != expected_parent
            ));

            artifact = artifact.apply_change(&change).unwrap();
            drop(view);
            workspace.cleanup().await.unwrap();

            let mut restored = materialized_workspace(&fixture, &artifact).await;
            let restored_view = restored.view();
            assert_eq!(
                restored_view
                    .read_file(&workspace_path("repos/program/program.txt"))
                    .unwrap(),
                program_body.as_bytes()
            );
            assert_eq!(
                restored_view
                    .read_file(&workspace_path("repos/bench/bench.txt"))
                    .unwrap(),
                bench_body.as_bytes()
            );
            drop(restored_view);
            restored.cleanup().await.unwrap();
        }
    });
}

struct GitFixture {
    program_store: PathBuf,
    bench_store: PathBuf,
    program_parent: GitRevision,
    program_tree: GitRevision,
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
        let program_tree = GitRevision::Tree(git_object(
            git_output(&program_source, ["rev-parse", "main^{tree}"]).trim(),
        ));
        let bench_parent = GitRevision::Commit(git_object(
            git_output(&bench_source, ["rev-parse", "main"]).trim(),
        ));

        Self {
            program_store,
            bench_store,
            program_parent,
            program_tree,
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
                (repo_key("program"), self.program_repo_artifact()),
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

    fn program_artifact(&self) -> GitProgramArtifact {
        GitProgramArtifact::new(
            BTreeMap::from([(repo_key("program"), self.program_repo_artifact())]),
            GitProgramLayout::new(BTreeMap::from([(
                repo_key("program"),
                git_path("repos/program"),
            )]))
            .unwrap(),
        )
        .unwrap()
    }

    fn program_tree_artifact(&self) -> GitProgramArtifact {
        GitProgramArtifact::new(
            BTreeMap::from([(
                repo_key("program"),
                GitRepoArtifact::new(
                    RepoRef::global(repo_key("program")),
                    self.program_tree.clone(),
                    None,
                    GitArtifactIdentityMode::Tree,
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

    fn program_repo_artifact(&self) -> GitRepoArtifact {
        GitRepoArtifact::new(
            RepoRef::global(repo_key("program")),
            self.program_parent.clone(),
            None,
            GitArtifactIdentityMode::Commit,
        )
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

async fn materialized_workspace_without_local_mount(
    fixture: &GitFixture,
    artifact: &GitProgramArtifact,
) -> leaven_workspace::Workspace {
    let root = tempfile::tempdir().unwrap().keep();
    let mut workspace = NoLocalMountWorkspaceFactory::new(&root)
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

fn configure_workspace_git(view: &mut leaven_workspace::WorkspaceView<'_>, cwd: &str) {
    workspace_git(view, cwd, ["config", "user.name", "Leaven Test"]);
    workspace_git(
        view,
        cwd,
        ["config", "user.email", "leaven@example.invalid"],
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

fn workspace_command<const N: usize>(
    view: &mut leaven_workspace::WorkspaceView<'_>,
    cwd: Option<&str>,
    program: &str,
    args: [&str; N],
) {
    let mut command = WorkspaceCommand::new(program);
    command.cwd = cwd.map(workspace_path);
    command.args = args.into_iter().map(str::to_owned).collect();
    let output = view.run_command(command).unwrap();
    assert!(
        output.status.code == Some(0),
        "{program} failed: {}",
        String::from_utf8_lossy(&output.stderr.bytes)
    );
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

#[derive(Clone, Debug)]
struct NoLocalMountWorkspaceFactory {
    root: PathBuf,
    rejected_command_fragments: Arc<Vec<String>>,
}

impl NoLocalMountWorkspaceFactory {
    fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            rejected_command_fragments: Arc::new(Vec::new()),
        }
    }

    fn rejecting_command_fragments(mut self, fragments: Vec<String>) -> Self {
        self.rejected_command_fragments = Arc::new(fragments);
        self
    }
}

impl WorkspaceFactory for NoLocalMountWorkspaceFactory {
    async fn allocate(&self, _config: WorkspaceConfig) -> Result<Workspace, FactoryError> {
        fs::create_dir_all(&self.root)
            .map_err(|error| FactoryError::Allocate(error.to_string()))?;
        Ok(Workspace::new(
            self.root.clone(),
            Box::new(NoLocalMountBackend {
                root: self.root.clone(),
                rejected_command_fragments: self.rejected_command_fragments.clone(),
            }),
        ))
    }
}

struct NoLocalMountBackend {
    root: PathBuf,
    rejected_command_fragments: Arc<Vec<String>>,
}

impl WorkspaceBackend for NoLocalMountBackend {
    fn write_file(&mut self, path: &WorkspacePath, bytes: &[u8]) -> Result<(), WorkspaceError> {
        let path = self.host_path(path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| WorkspaceError::Io(error.to_string()))?;
        }
        fs::write(path, bytes).map_err(|error| WorkspaceError::Io(error.to_string()))
    }

    fn read_file(&mut self, path: &WorkspacePath) -> Result<Vec<u8>, WorkspaceError> {
        fs::read(self.host_path(path)).map_err(|error| WorkspaceError::Io(error.to_string()))
    }

    fn list_files(&mut self, path: &WorkspacePath) -> Result<Vec<WorkspacePath>, WorkspaceError> {
        let root = self.host_path(path);
        let mut files = Vec::new();
        collect_no_mount_files(&root, path.clone(), &mut files)?;
        files.sort();
        Ok(files)
    }

    fn run_command(
        &mut self,
        command: WorkspaceCommand,
    ) -> Result<leaven_workspace::CommandOutput, WorkspaceError> {
        if let Some(fragment) =
            command_forbidden_fragment(&command, &self.rejected_command_fragments)
        {
            return Err(WorkspaceError::Command(format!(
                "workspace command exposed forbidden host path fragment `{fragment}`"
            )));
        }
        if command.user.is_some() {
            return Err(WorkspaceError::UnsupportedOperation {
                operation: "run_command.user",
            });
        }
        let cwd = command
            .cwd
            .as_ref()
            .map_or_else(|| self.root.clone(), |path| self.host_path(path));
        let start = Instant::now();
        let mut process = ProcessCommand::new(&command.program);
        process
            .args(&command.args)
            .current_dir(cwd)
            .envs(&command.env)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        match &command.stdin {
            CommandStdin::Empty => {
                process.stdin(Stdio::null());
            }
            CommandStdin::Bytes(_) => {
                process.stdin(Stdio::piped());
            }
        }
        let mut child = process
            .spawn()
            .map_err(|error| WorkspaceError::Command(error.to_string()))?;
        if let CommandStdin::Bytes(bytes) = &command.stdin
            && let Some(mut stdin) = child.stdin.take()
        {
            stdin
                .write_all(bytes)
                .map_err(|error| WorkspaceError::Command(error.to_string()))?;
        }
        let output = child
            .wait_with_output()
            .map_err(|error| WorkspaceError::Command(error.to_string()))?;
        Ok(leaven_workspace::CommandOutput {
            status: ExitStatus {
                code: output.status.code(),
            },
            stdout: CapturedOutput::new(output.stdout, command.limits.max_stdout_bytes),
            stderr: CapturedOutput::new(output.stderr, command.limits.max_stderr_bytes),
            output_files: std::collections::BTreeMap::new(),
            duration: start.elapsed(),
        })
    }

    fn cleanup(self: Box<Self>) -> BoxFuture<'static, Result<(), WorkspaceError>> {
        async move {
            if self.root.exists() {
                fs::remove_dir_all(&self.root)
                    .map_err(|error| WorkspaceError::Cleanup(error.to_string()))?;
            }
            Ok(())
        }
        .boxed()
    }
}

impl NoLocalMountBackend {
    fn host_path(&self, path: &WorkspacePath) -> PathBuf {
        self.root.join(path.to_host_relative())
    }
}

fn command_forbidden_fragment<'a>(
    command: &WorkspaceCommand,
    fragments: &'a [String],
) -> Option<&'a str> {
    if fragments.is_empty() {
        return None;
    }
    let cwd = command.cwd.as_ref().map(WorkspacePath::as_str);
    fragments
        .iter()
        .find(|fragment| {
            string_contains_fragment(&command.program, fragment)
                || command
                    .args
                    .iter()
                    .any(|arg| string_contains_fragment(arg, fragment))
                || command.env.iter().any(|(key, value)| {
                    string_contains_fragment(key, fragment)
                        || string_contains_fragment(value, fragment)
                })
                || cwd.is_some_and(|cwd| string_contains_fragment(cwd, fragment))
        })
        .map(String::as_str)
}

fn string_contains_fragment(value: &str, fragment: &str) -> bool {
    !fragment.is_empty() && value.contains(fragment)
}

fn collect_no_mount_files(
    host_path: &Path,
    workspace_path: WorkspacePath,
    files: &mut Vec<WorkspacePath>,
) -> Result<(), WorkspaceError> {
    let metadata =
        fs::metadata(host_path).map_err(|error| WorkspaceError::Io(error.to_string()))?;
    if metadata.is_file() {
        files.push(workspace_path);
        return Ok(());
    }
    for entry in fs::read_dir(host_path).map_err(|error| WorkspaceError::Io(error.to_string()))? {
        let entry = entry.map_err(|error| WorkspaceError::Io(error.to_string()))?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| WorkspaceError::Io("workspace path is not UTF-8".to_owned()))?;
        let child_path = if workspace_path.as_str().is_empty() {
            WorkspacePath::new(name)?
        } else {
            workspace_path.join(name)?
        };
        collect_no_mount_files(&entry.path(), child_path, files)?;
    }
    Ok(())
}
