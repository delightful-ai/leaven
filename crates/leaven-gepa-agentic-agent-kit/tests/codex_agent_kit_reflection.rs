use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use leaven_agentic_agent_kit::{AgentKitMountMode, CodexAgentKitMaterializer};
use leaven_agentic_git::{GitProgramMaterializer, GitProgramReadback, GitProgramStores};
use leaven_artifact_git::{
    GitArtifactIdentityMode, GitObjectId, GitPath, GitProgramArtifact, GitProgramChange,
    GitProgramLayout, GitRepoArtifact, GitRevision, RepoKey, RepoRef,
};
use leaven_core::{InfoRef, OptimizationProblem, Proposal, ProposalBatch, ProposalBatchSemantics};
use leaven_engine::{
    Arity, BudgetLedger, ProposalContext, ProposalError, Proposer, RunContext, RunGraph,
};
use leaven_gepa::ReflectRequest;
use leaven_gepa_agentic_agent_kit::{
    AgentKitReflectionPart, CodexAgentKitReflectionInput, CodexAgentKitReflectionSmoke,
};
use leaven_kernel::{Budget, Cost, MetadataBag, Metered, ProposerId, RunId};
use leaven_workspace::{Command, WorkspaceConfig, WorkspaceFactory, WorkspacePath};
use leaven_workspace_local::LocalWorkspaceFactory;

#[test]
fn codex_agent_kit_reflection_projects_system_prompt_and_applies_git_child() {
    futures::executor::block_on(async {
        let temp = tempfile::tempdir().unwrap();
        let kit = temp.path().join("repo/agent");
        write_agent_kit_with_hook_scaffold(&kit);
        let workspace = temp.path().join("workspace");
        let program = program_artifact(commit("11"));

        let mut graph = RunGraph::<AgentKitGitProblem>::new(RunId::new());
        let mut budget = BudgetLedger::new(Budget::unlimited());
        let parent = {
            let mut ctx = RunContext::<AgentKitGitProblem>::new(&mut graph, &mut budget);
            ctx.insert_seed(program.clone(), 0).unwrap()
        };
        let request = ReflectRequest::for_part(
            parent,
            AgentKitReflectionPart::SystemPrompt,
            "system_prompt.md",
        )
        .with_source_refs([InfoRef::Candidate(parent)]);
        let smoke = CodexAgentKitReflectionSmoke::new(AgentKitMountMode::Copy);
        let report = smoke
            .project_and_import_change(
                &kit,
                &workspace,
                CodexAgentKitReflectionInput::new(program, repo_key("agent"), request.clone()),
                commit("33"),
            )
            .unwrap();

        assert_eq!(
            report.materialization.system_prompt.as_deref(),
            Some("Prefer durable repo identity.\n")
        );
        assert!(report.system_prompt_targeted);
        assert!(!report.agent_docs_targeted);
        assert!(report.hook_scaffold_ignored);
        assert_eq!(
            fs::read_to_string(workspace.join("AGENTS.md")).unwrap(),
            "Keep system_prompt.md separate.\n"
        );
        assert!(matches!(
            report.change,
            GitProgramChange::AdvanceRepo { .. }
        ));

        let proposal = Proposal::mutate(parent, report.change.clone())
            .informed_by(request.informed_by())
            .build();
        let batch = ProposalBatch {
            proposals: vec![proposal],
            semantics: ProposalBatchSemantics::Alternatives,
            metadata: MetadataBag::new(),
        };
        let child = {
            let mut ctx = RunContext::<AgentKitGitProblem>::new(&mut graph, &mut budget);
            let proposed = ctx
                .propose(&PrebuiltProposalProposer::new(batch), ())
                .await
                .unwrap();
            ctx.apply_batch(proposed.batch_id)
                .unwrap()
                .successful_candidates()
                .next()
                .unwrap()
        };
        let ctx = RunContext::<AgentKitGitProblem>::new(&mut graph, &mut budget);
        assert_eq!(ctx.graph().parents(child), vec![parent]);
        assert_eq!(
            ctx.graph()
                .artifact(child)
                .unwrap()
                .repo(&repo_key("agent"))
                .unwrap()
                .revision(),
            &commit("33")
        );
        assert!(
            ctx.graph()
                .proposal_that_created(child)
                .unwrap()
                .provenance()
                .informed_by_refs()
                .contains(&InfoRef::Candidate(parent))
        );
    });
}

#[test]
fn codex_agent_kit_reflection_reads_back_git_workspace_child_before_next_projection() {
    futures::executor::block_on(async {
        let fixture = AgentKitRepoFixture::new();
        let parent_artifact = fixture.program_artifact();
        let stores = fixture.stores();
        let mut first_workspace = LocalWorkspaceFactory::temp()
            .allocate(WorkspaceConfig::default())
            .await
            .unwrap();
        let mut view = first_workspace.view();
        GitProgramMaterializer::new(stores.clone())
            .materialize_program(&parent_artifact, &mut view)
            .unwrap();
        let first_root = view.local_mount().unwrap().to_path_buf();

        let projected = CodexAgentKitMaterializer::new(AgentKitMountMode::Copy)
            .materialize(first_root.join("repos/agent"), &first_root)
            .unwrap();
        assert_eq!(
            projected.system_prompt.as_deref(),
            Some("Prefer parent behavior.\n")
        );
        assert!(first_root.join(".agents/skills/alpha/SKILL.md").exists());

        write_workspace_file(
            &mut view,
            "repos/agent/system_prompt.md",
            "Prefer applied child behavior.\n",
        );
        write_workspace_file(
            &mut view,
            "repos/agent/skills/alpha/SKILL.md",
            "---\nname: alpha\ndescription: Alpha skill.\n---\n\nDo applied child work.\n",
        );
        workspace_git(&mut view, "repos/agent", ["add", "."]);
        workspace_git(
            &mut view,
            "repos/agent",
            ["commit", "-m", "evolve agent kit"],
        );

        let change = GitProgramReadback::new(stores.clone())
            .read_back_change(&parent_artifact, &mut view)
            .unwrap()
            .expect("dirty AgentKit repo must read back a typed GitProgramChange");

        let mut graph = RunGraph::<AgentKitGitProblem>::new(RunId::new());
        let mut budget = BudgetLedger::new(Budget::unlimited());
        let parent = {
            let mut ctx = RunContext::<AgentKitGitProblem>::new(&mut graph, &mut budget);
            ctx.insert_seed(parent_artifact.clone(), 0).unwrap()
        };
        let proposal = Proposal::mutate(parent, change.clone())
            .informed_by([InfoRef::Candidate(parent)])
            .build();
        let batch = ProposalBatch {
            proposals: vec![proposal],
            semantics: ProposalBatchSemantics::Alternatives,
            metadata: MetadataBag::new(),
        };
        let child = {
            let mut ctx = RunContext::<AgentKitGitProblem>::new(&mut graph, &mut budget);
            let proposed = ctx
                .propose(&PrebuiltProposalProposer::new(batch), ())
                .await
                .unwrap();
            ctx.apply_batch(proposed.batch_id)
                .unwrap()
                .successful_candidates()
                .next()
                .unwrap()
        };
        let child_artifact = {
            let ctx = RunContext::<AgentKitGitProblem>::new(&mut graph, &mut budget);
            assert_eq!(ctx.graph().parents(child), vec![parent]);
            ctx.graph().artifact(child).unwrap().clone()
        };

        drop(view);
        first_workspace.cleanup().await.unwrap();

        let mut second_workspace = LocalWorkspaceFactory::temp()
            .allocate(WorkspaceConfig::default())
            .await
            .unwrap();
        let mut second_view = second_workspace.view();
        GitProgramMaterializer::new(stores)
            .materialize_program(&child_artifact, &mut second_view)
            .unwrap();
        let second_root = second_view.local_mount().unwrap().to_path_buf();
        let next_projection = CodexAgentKitMaterializer::new(AgentKitMountMode::Copy)
            .materialize(second_root.join("repos/agent"), &second_root)
            .unwrap();

        assert_eq!(
            next_projection.system_prompt.as_deref(),
            Some("Prefer applied child behavior.\n")
        );
        assert_eq!(
            fs::read_to_string(second_root.join(".agents/skills/alpha/SKILL.md")).unwrap(),
            "---\nname: alpha\ndescription: Alpha skill.\n---\n\nDo applied child work.\n"
        );

        drop(second_view);
        second_workspace.cleanup().await.unwrap();
    });
}

#[derive(Clone, Debug)]
struct AgentKitGitProblem;

impl OptimizationProblem for AgentKitGitProblem {
    type Artifact = GitProgramArtifact;
    type Case = ();
    type Evidence = leaven_evidence::ScalarEvidence;
    type ProposalAnnotations = ();
}

#[derive(Clone)]
struct PrebuiltProposalProposer {
    batch: ProposalBatch<AgentKitGitProblem>,
}

impl PrebuiltProposalProposer {
    fn new(batch: ProposalBatch<AgentKitGitProblem>) -> Self {
        Self { batch }
    }
}

impl Proposer<AgentKitGitProblem> for PrebuiltProposalProposer {
    type Request = ();

    fn id(&self) -> ProposerId {
        ProposerId::from("gepa/agent-kit-smoke")
    }

    fn arity(&self) -> Arity {
        Arity::Single
    }

    async fn propose(
        &self,
        _request: Self::Request,
        _ctx: ProposalContext<'_, AgentKitGitProblem>,
    ) -> Result<Metered<ProposalBatch<AgentKitGitProblem>>, ProposalError> {
        Ok(Metered::new(self.batch.clone(), Cost::zero()))
    }
}

fn write_agent_kit_with_hook_scaffold(root: &Path) {
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
        "Prefer durable repo identity.\n",
    )
    .unwrap();
    fs::write(root.join("AGENTS.md"), "Keep system_prompt.md separate.\n").unwrap();
    fs::write(
        root.join("skills/alpha/SKILL.md"),
        "---\nname: alpha\ndescription: Alpha skill.\n---\n\nDo alpha work.\n",
    )
    .unwrap();
    fs::write(root.join("hooks/pre-run.sh"), "exit 1\n").unwrap();
}

struct AgentKitRepoFixture {
    _temp: tempfile::TempDir,
    bare_store: std::path::PathBuf,
    parent_commit: GitObjectId,
}

impl AgentKitRepoFixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("agent-source");
        let bare_store = temp.path().join("agent.git");
        fs::create_dir_all(&source).unwrap();
        run_host_git(&source, ["init", "--initial-branch=main"]);
        run_host_git(&source, ["config", "user.name", "Leaven Test"]);
        run_host_git(&source, ["config", "user.email", "leaven@example.invalid"]);
        write_agent_kit_repo(&source);
        run_host_git(&source, ["add", "."]);
        run_host_git(&source, ["commit", "-m", "seed agent kit"]);
        let parent_commit = git_object(host_git_output(&source, ["rev-parse", "HEAD"]).trim());
        let parent = temp.path();
        run_host_git_at(
            parent,
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

    fn stores(&self) -> GitProgramStores {
        GitProgramStores::new(BTreeMap::from([(
            repo_key("agent"),
            self.bare_store.clone(),
        )]))
        .unwrap()
    }

    fn program_artifact(&self) -> GitProgramArtifact {
        program_artifact(GitRevision::Commit(self.parent_commit.clone()))
    }
}

fn write_agent_kit_repo(root: &Path) {
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
    fs::write(root.join("system_prompt.md"), "Prefer parent behavior.\n").unwrap();
    fs::write(root.join("AGENTS.md"), "Keep child projection visible.\n").unwrap();
    fs::write(
        root.join("skills/alpha/SKILL.md"),
        "---\nname: alpha\ndescription: Alpha skill.\n---\n\nDo parent work.\n",
    )
    .unwrap();
    fs::write(root.join("hooks/pre-run.sh"), "exit 1\n").unwrap();
}

fn write_workspace_file(view: &mut leaven_workspace::WorkspaceView<'_>, path: &str, body: &str) {
    view.write_file(&workspace_path(path), body.as_bytes())
        .unwrap();
}

fn workspace_git<const N: usize>(
    view: &mut leaven_workspace::WorkspaceView<'_>,
    cwd: &str,
    args: [&str; N],
) {
    let mut command = Command::new("git");
    command.cwd = Some(workspace_path(cwd));
    command.args = args.iter().map(|arg| (*arg).to_owned()).collect();
    let output = view.run_command(command).unwrap();
    assert!(
        output.status.code == Some(0),
        "git {} failed: {}",
        args.join(" "),
        String::from_utf8_lossy(&output.stderr.bytes)
    );
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

fn commit(byte: &str) -> GitRevision {
    GitRevision::commit(format!("{byte:0<40}")).unwrap()
}

fn workspace_path(path: &str) -> WorkspacePath {
    WorkspacePath::new(path).unwrap()
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
