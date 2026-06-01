use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use leaven_agentic_agent_kit::AgentKitMountMode;
use leaven_artifact_git::{
    GitArtifactIdentityMode, GitPath, GitProgramArtifact, GitProgramChange, GitProgramLayout,
    GitRepoArtifact, GitRevision, RepoKey, RepoRef,
};
use leaven_core::{
    InfoRef, OptimizationProblem, Proposal, ProposalBatch, ProposalBatchSemantics,
};
use leaven_engine::{
    Arity, BudgetLedger, ProposalContext, ProposalError, Proposer, RunContext, RunGraph,
};
use leaven_gepa::ReflectRequest;
use leaven_gepa_agentic_agent_kit::{
    AgentKitReflectionPart, CodexAgentKitReflectionInput, CodexAgentKitReflectionSmoke,
};
use leaven_kernel::{Budget, Cost, MetadataBag, Metered, ProposerId, RunId};

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
    fs::write(root.join("system_prompt.md"), "Prefer durable repo identity.\n").unwrap();
    fs::write(root.join("AGENTS.md"), "Keep system_prompt.md separate.\n").unwrap();
    fs::write(
        root.join("skills/alpha/SKILL.md"),
        "---\nname: alpha\ndescription: Alpha skill.\n---\n\nDo alpha work.\n",
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

fn commit(byte: &str) -> GitRevision {
    GitRevision::commit(format!("{byte:0<40}")).unwrap()
}
