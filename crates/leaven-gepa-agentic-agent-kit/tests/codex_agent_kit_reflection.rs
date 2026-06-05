use std::collections::BTreeMap;
use std::fs;
use std::io::Cursor;
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
use leaven_seam_runtime::SeamRuntime;
use leaven_seam_service::RunBoundGraphEffectService;
use leaven_seam_stdio::serve_reader_writer;
use leaven_workspace::{Command, WorkspaceConfig, WorkspaceFactory, WorkspacePath};
use leaven_workspace_local::LocalWorkspaceFactory;
use serde_json::{Value, json};

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

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "single scenario proves AgentKit stdio submit/apply before next projection"
)]
fn codex_agent_kit_git_child_applies_through_run_bound_stdio_before_next_projection() {
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
        write_workspace_file(
            &mut view,
            "repos/agent/system_prompt.md",
            "Prefer stdio-applied child behavior.\n",
        );
        write_workspace_file(
            &mut view,
            "repos/agent/skills/alpha/SKILL.md",
            "---\nname: alpha\ndescription: Alpha skill.\n---\n\nDo stdio-applied work.\n",
        );
        workspace_git(&mut view, "repos/agent", ["add", "."]);
        workspace_git(
            &mut view,
            "repos/agent",
            ["commit", "-m", "evolve agent kit through seam"],
        );
        let change = GitProgramReadback::new(stores.clone())
            .read_back_change(&parent_artifact, &mut view)
            .unwrap()
            .expect("changed AgentKit repo must import a typed GitProgramChange");
        drop(view);
        first_workspace.cleanup().await.unwrap();

        let mut graph = RunGraph::<AgentKitGitProblem>::new(RunId::new());
        let mut budget = BudgetLedger::new(Budget::unlimited());
        let parent = {
            let mut ctx = RunContext::<AgentKitGitProblem>::new(&mut graph, &mut budget);
            ctx.insert_seed(parent_artifact, 0).unwrap()
        };
        {
            let mut ctx = RunContext::<AgentKitGitProblem>::new(&mut graph, &mut budget);
            let service = RunBoundGraphEffectService::new(
                &mut ctx,
                [],
                "fp_cap_sha256_agent_kit",
                "fp_policy_sha256_agent_kit",
                "rev_agent_kit_base",
                "rev_agent_kit_child",
            )
            .with_proposal_submitter({
                move |params| {
                    assert_eq!(params.plan_id(), "plan_agent_kit_submit");
                    assert_eq!(params.op_name(), "proposal_batch");
                    let proposal = params.proposals().first().expect("proposal is present");
                    assert!(proposal.effect().is_change_from_agent_session());
                    Ok(ProposalBatch {
                        proposals: vec![
                            Proposal::mutate(parent, change.clone())
                                .informed_by([InfoRef::Candidate(parent)])
                                .build(),
                        ],
                        semantics: ProposalBatchSemantics::Alternatives,
                        metadata: MetadataBag::new(),
                    })
                }
            });
            let package = leaven_public_seam::PublicSeamPackage::active_from_repo(workspace_root())
                .expect("public seam package loads from workspace");
            let runtime = SeamRuntime::from_package(package, service).unwrap();
            let mut submit_output = Vec::new();
            let submit_report = serve_reader_writer(
                &runtime,
                Cursor::new(format!(
                    "{}\n",
                    jsonrpc_request(
                        "agent-kit-submit",
                        "leaven/proposal.submit_batch",
                        &proposal_submit_request(),
                    )
                )),
                &mut submit_output,
            )
            .unwrap();
            assert_eq!(submit_report.requests, 1);
            let submit_lines = response_lines(submit_output);
            assert!(
                submit_lines.iter().all(|line| line.get("error").is_none()),
                "stdio runtime returned submit errors: {submit_lines:?}"
            );
            assert_eq!(
                submit_lines[0]["result"]["primary"]["kind"],
                "proposal_batch_receipt"
            );
            assert_eq!(
                submit_lines[0]["result"]["receipts"][0]["write_kind"],
                "submit_proposal_batch"
            );
            let batch_ref = submit_lines[0]["result"]["primary"]["batch_id"]
                .as_str()
                .expect("proposal.submit_batch returns batch id")
                .to_owned();

            let mut apply_output = Vec::new();
            let apply_report = serve_reader_writer(
                &runtime,
                Cursor::new(format!(
                    "{}\n",
                    jsonrpc_request(
                        "agent-kit-apply",
                        "leaven/proposal.apply",
                        &proposal_apply_request(&batch_ref),
                    )
                )),
                &mut apply_output,
            )
            .unwrap();
            assert_eq!(apply_report.requests, 1);
            let lines = response_lines(apply_output);
            assert!(
                lines.iter().all(|line| line.get("error").is_none()),
                "stdio runtime returned error responses: {lines:?}"
            );
            assert_eq!(lines[0]["result"]["primary"]["kind"], "apply_receipt");
            assert_eq!(
                lines[0]["result"]["primary"]["graph_revision"],
                "rev_agent_kit_child"
            );
            assert_eq!(
                lines[0]["result"]["receipts"][0]["write_kind"],
                "apply_proposal_batch"
            );
        }

        let child_artifact = {
            let ctx = RunContext::<AgentKitGitProblem>::new(&mut graph, &mut budget);
            let children = ctx.graph().children(parent);
            assert_eq!(children.len(), 1, "stdio apply must create one graph child");
            let child = children[0];
            ctx.graph().artifact(child).unwrap().clone()
        };
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
            Some("Prefer stdio-applied child behavior.\n")
        );
        assert_eq!(
            fs::read_to_string(second_root.join(".agents/skills/alpha/SKILL.md")).unwrap(),
            "---\nname: alpha\ndescription: Alpha skill.\n---\n\nDo stdio-applied work.\n"
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

fn jsonrpc_request(id: &str, method: &str, params: &Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": method,
        "params": params,
    })
}

fn response_lines(output: Vec<u8>) -> Vec<Value> {
    String::from_utf8(output)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect()
}

fn proposal_submit_request() -> Value {
    json!({
        "schema_version": "leaven.plan.v1",
        "plan_id": "plan_agent_kit_submit",
        "consistency": {
            "kind": "latest_at_start"
        },
        "mode": {
            "kind": "execute"
        },
        "ops": [{
            "kind": "write",
            "name": "proposal_batch",
            "idempotency_key": "agent-kit-submit-0001",
            "write": {
                "kind": "submit_proposal_batch",
                "semantics": "sequence",
                "proposals": [{
                    "effect": {
                        "kind": "change_from_agent_session",
                        "target": "cand_agent_kit_parent",
                        "agent_receipt": "agentrec_codex",
                        "parser": "leaven.agent_session.skill_patch.v1",
                        "surface_fingerprint": "fp_surface_sha256_agent_kit",
                        "change_schema": "fp_schema_sha256_agent_kit_change"
                    },
                    "causal": {
                        "inputs": ["cand_agent_kit_parent"]
                    },
                    "informed_by": {
                        "kind": "literal",
                        "value": ["qrec_agent_kit_parent", "agentrec_codex"]
                    },
                    "read_receipts": ["qrec_agent_kit_parent", "agentrec_codex"]
                }]
            }
        }],
        "return": ["proposal_batch"],
        "commit": {
            "kind": "graph_writes_atomic",
            "on_stale": "reject"
        }
    })
}

fn proposal_apply_request(batch_ref: &str) -> Value {
    json!({
        "schema_version": "leaven.plan.v1",
        "plan_id": "plan_agent_kit_apply",
        "consistency": {
            "kind": "latest_at_start"
        },
        "mode": {
            "kind": "execute"
        },
        "ops": [{
            "kind": "write",
            "name": "apply",
            "idempotency_key": "agent-kit-apply-0001",
            "write": {
                "kind": "apply_proposal_batch",
                "proposal_batch": batch_ref,
                "policy": "apply_first_valid"
            }
        }],
        "return": ["apply"],
        "commit": {
            "kind": "graph_writes_atomic",
            "on_stale": "reject"
        }
    })
}

fn workspace_root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .unwrap()
        .to_path_buf()
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
