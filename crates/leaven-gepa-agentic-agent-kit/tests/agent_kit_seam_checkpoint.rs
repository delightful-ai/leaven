use std::collections::BTreeMap;
use std::fs;
use std::io::Cursor;
use std::path::Path;

use bytes::Bytes;
use leaven_agentic_agent_kit::{AgentKitMountMode, CodexAgentKitMaterializer};
use leaven_agentic_git::{GitProgramMaterializer, GitProgramReadback, GitProgramStores};
use leaven_artifact_git::{
    GitArtifactIdentityMode, GitObjectId, GitPath, GitProgramArtifact, GitProgramLayout,
    GitRepoArtifact, GitRevision, RepoKey, RepoRef,
};
use leaven_core::{
    Evidence, InfoRef, OptimizationProblem, Proposal, ProposalBatch, ProposalBatchSemantics,
};
use leaven_engine::{
    CaseSet, Optimizer, OptimizerError, RunContext, StepStatus, StoreRunPersistence,
};
use leaven_kernel::{Budget, CandidateId, MetadataBag};
use leaven_seam_run::RunBoundSdkRoute;
use leaven_seam_service::RunBoundGraphEffectService;
use leaven_store::{BlobStore, BlobWrite};
use leaven_store_file::FileStore;
use leaven_store_inline::InlineEvidenceStore;
use leaven_workspace::{Command, WorkspaceConfig, WorkspaceFactory, WorkspacePath};
use leaven_workspace_local::LocalWorkspaceFactory;
use serde_json::{Value, json};

#[test]
fn agent_kit_git_child_applied_through_sdk_route_restores_from_checkpoint() {
    futures::executor::block_on(async {
        let run_dir = tempfile::tempdir().unwrap();
        let store = FileStore::open(run_dir.path()).unwrap();
        let persistence = StoreRunPersistence::new(store.clone());
        let fixture = AgentKitRepoFixture::new();
        let parent_artifact = fixture.program_artifact();
        let stores = fixture.stores();
        let mut engine = leaven_engine::Engine::<AgentKitCheckpointProblem>::builder()
            .budget(Budget::unlimited())
            .persistence(persistence.clone())
            .build();
        let seed = engine.insert_seed(parent_artifact.clone(), 0).unwrap();
        let case_set = CaseSet::new(Vec::<AgentKitCase>::new());
        let evidence_store = InlineEvidenceStore::<AgentKitEvidence>::new("agent-kit");
        let mut optimizer = AgentKitCheckpointOptimizer {
            seed,
            parent_artifact,
            stores: stores.clone(),
            store: store.clone(),
            mounted: false,
        };

        engine
            .run(&mut optimizer, &case_set, &evidence_store)
            .await
            .unwrap();
        assert!(optimizer.mounted, "optimizer must mount the SDK route");

        let export = leaven_run::export_local_run_inspection(run_dir.path()).unwrap();
        assert_eq!(export.checkpoint.stage_journal_ref_count, 1);
        let transcript = &export.checkpoint.stage_journal_refs[0];
        let transcript_blob =
            leaven_run::export_local_run_blob(run_dir.path(), &transcript.store, &transcript.key)
                .unwrap();
        assert_eq!(transcript_blob.bytes, AGENT_KIT_TRANSCRIPT_BYTES.len());
        assert!(!transcript_blob.content_base64.is_empty());
        let restored = persistence
            .latest_checkpoint::<AgentKitCheckpointProblem>()
            .unwrap()
            .expect("run should write a latest checkpoint");
        assert_eq!(export.run_id, restored.checkpoint.run_id);
        assert_eq!(export.graph.candidate_count, 2);
        assert_eq!(export.graph.proposal_count, 1);
        assert_eq!(export.graph.apply_attempt_count, 1);
        let mut restored_graph = restored.graph;
        let mut restored_budget = restored.budget;
        let restored_ctx =
            RunContext::<AgentKitCheckpointProblem>::new(&mut restored_graph, &mut restored_budget);
        let child = restored_ctx.graph().children(seed)[0];
        let child_artifact = restored_ctx.graph().artifact(child).unwrap().clone();

        let mut workspace = LocalWorkspaceFactory::temp()
            .allocate(WorkspaceConfig::default())
            .await
            .unwrap();
        let mut view = workspace.view();
        GitProgramMaterializer::new(stores)
            .materialize_program(&child_artifact, &mut view)
            .unwrap();
        let root = view.local_mount().unwrap().to_path_buf();
        let next_projection = CodexAgentKitMaterializer::new(AgentKitMountMode::Copy)
            .materialize(root.join("repos/agent"), &root)
            .unwrap();
        assert_eq!(
            next_projection.system_prompt.as_deref(),
            Some("Prefer checkpoint-restored child behavior.\n")
        );
        assert_eq!(
            fs::read_to_string(root.join(".agents/skills/alpha/SKILL.md")).unwrap(),
            "---\nname: alpha\ndescription: Alpha skill.\n---\n\nDo checkpoint-restored work.\n"
        );
        drop(view);
        workspace.cleanup().await.unwrap();
    });
}

struct AgentKitCheckpointOptimizer {
    seed: CandidateId,
    parent_artifact: GitProgramArtifact,
    stores: GitProgramStores,
    store: FileStore,
    mounted: bool,
}

impl Optimizer<AgentKitCheckpointProblem> for AgentKitCheckpointOptimizer {
    async fn step(
        &mut self,
        ctx: &mut RunContext<'_, AgentKitCheckpointProblem>,
    ) -> Result<StepStatus, OptimizerError> {
        let change = read_back_agent_kit_change(&self.parent_artifact, self.stores.clone()).await?;
        let transcript_ref = BlobStore::put(
            &self.store,
            BlobWrite {
                bytes: Bytes::from_static(AGENT_KIT_TRANSCRIPT_BYTES),
                content_type: Some("application/json".to_owned()),
            },
        )
        .map_err(|source| OptimizerError::with_source("write AgentKit transcript blob", source))?;
        ctx.record_stage_journal_entry(transcript_ref)
            .map_err(|source| {
                OptimizerError::with_source("record AgentKit transcript checkpoint ref", source)
            })?;
        let seed = self.seed;
        let service = RunBoundGraphEffectService::new(
            ctx,
            [],
            "fp_cap_sha256_agent_kit_checkpoint",
            "fp_policy_sha256_agent_kit_checkpoint",
            "rev_agent_kit_checkpoint_base",
            "rev_agent_kit_checkpoint_child",
        )
        .with_proposal_submitter({
            move |params| {
                if params.plan_id() != "plan_agent_kit_checkpoint_submit" {
                    return Err(format!("unexpected AgentKit plan {}", params.plan_id()));
                }
                if params.proposals_payload()[0]["effect"]["kind"] != "change_from_agent_session" {
                    return Err("unexpected AgentKit proposal effect".to_owned());
                }
                Ok(ProposalBatch {
                    proposals: vec![
                        Proposal::mutate(seed, change.clone())
                            .informed_by([InfoRef::Candidate(seed)])
                            .build(),
                    ],
                    semantics: ProposalBatchSemantics::Alternatives,
                    metadata: MetadataBag::new(),
                })
            }
        });
        let route = RunBoundSdkRoute::bind_run_bound_service(workspace_root(), service)
            .map_err(|source| OptimizerError::with_source("bind AgentKit SDK route", source))?;
        let submit = serve_jsonrpc_lines(
            &route,
            [jsonrpc_request(
                "agent-kit-checkpoint-submit",
                "leaven/proposal.submit_batch",
                &submit_request(),
            )],
        )?;
        assert_success(&submit[0], "leaven/proposal.submit_batch")?;
        let batch_ref = submit[0]["result"]["primary"]["batch_id"]
            .as_str()
            .ok_or_else(|| {
                OptimizerError::Message("AgentKit submit response missing batch id".to_owned())
            })?
            .to_owned();
        let apply = serve_jsonrpc_lines(
            &route,
            [jsonrpc_request(
                "agent-kit-checkpoint-apply",
                "leaven/proposal.apply",
                &apply_request(&batch_ref),
            )],
        )?;
        assert_success(&apply[0], "leaven/proposal.apply")?;
        self.mounted = true;
        Ok(StepStatus::Done)
    }

    fn best_candidate(
        &self,
        graph: leaven_engine::RunGraphView<'_, AgentKitCheckpointProblem>,
    ) -> Option<CandidateId> {
        graph
            .children(self.seed)
            .first()
            .copied()
            .or(Some(self.seed))
    }
}

const AGENT_KIT_TRANSCRIPT_BYTES: &[u8] =
    br#"{"kind":"agent_kit_checkpoint","edited":["system_prompt.md","skills/alpha/SKILL.md"]}"#;

async fn read_back_agent_kit_change(
    parent_artifact: &GitProgramArtifact,
    stores: GitProgramStores,
) -> Result<leaven_artifact_git::GitProgramChange, OptimizerError> {
    let mut workspace = LocalWorkspaceFactory::temp()
        .allocate(WorkspaceConfig::default())
        .await
        .map_err(|source| OptimizerError::with_source("allocate AgentKit workspace", source))?;
    let change = {
        let mut view = workspace.view();
        GitProgramMaterializer::new(stores.clone())
            .materialize_program(parent_artifact, &mut view)
            .map_err(|source| {
                OptimizerError::with_source("materialize AgentKit program", source)
            })?;
        write_workspace_file(
            &mut view,
            "repos/agent/system_prompt.md",
            "Prefer checkpoint-restored child behavior.\n",
        );
        write_workspace_file(
            &mut view,
            "repos/agent/skills/alpha/SKILL.md",
            "---\nname: alpha\ndescription: Alpha skill.\n---\n\nDo checkpoint-restored work.\n",
        );
        workspace_git(&mut view, "repos/agent", ["add", "."])?;
        workspace_git(
            &mut view,
            "repos/agent",
            ["commit", "-m", "evolve agent kit through checkpointed seam"],
        )?;
        GitProgramReadback::new(stores)
            .read_back_change(parent_artifact, &mut view)
            .map_err(|source| OptimizerError::with_source("read back AgentKit Git change", source))?
            .ok_or_else(|| {
                OptimizerError::Message("AgentKit readback found no change".to_owned())
            })?
    };
    workspace.cleanup().await.map_err(|source| {
        OptimizerError::with_source("cleanup AgentKit mutation workspace", source)
    })?;
    Ok(change)
}

fn assert_success(response: &Value, method: &str) -> Result<(), OptimizerError> {
    if response.get("error").is_some() {
        return Err(OptimizerError::Message(format!(
            "{method} returned JSON-RPC error: {response}"
        )));
    }
    if response["result"]["method"].as_str() != Some(method) {
        return Err(OptimizerError::Message(format!(
            "{method} response did not carry the method result: {response}"
        )));
    }
    Ok(())
}

fn serve_jsonrpc_lines<const N: usize>(
    route: &RunBoundSdkRoute<RunBoundGraphEffectService<'_, '_, AgentKitCheckpointProblem>>,
    requests: [Value; N],
) -> Result<Vec<Value>, OptimizerError> {
    let input = requests
        .into_iter()
        .map(|request| serde_json::to_string(&request))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|source| OptimizerError::with_source("serialize AgentKit route request", source))?
        .join("\n");
    let mut output = Vec::new();
    route
        .serve_reader_writer(Cursor::new(format!("{input}\n")), &mut output)
        .map_err(|source| OptimizerError::with_source("serve AgentKit route", source))?;
    String::from_utf8(output)
        .map_err(|source| OptimizerError::with_source("decode AgentKit route output", source))?
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|source| OptimizerError::with_source("parse AgentKit route response", source))
}

fn submit_request() -> Value {
    json!({
        "schema_version": "leaven.plan.v1",
        "plan_id": "plan_agent_kit_checkpoint_submit",
        "consistency": {"kind": "latest_at_start"},
        "mode": {"kind": "execute"},
        "ops": [{
            "kind": "write",
            "name": "proposal_batch",
            "idempotency_key": "agent-kit-checkpoint-submit-0001",
            "write": {
                "kind": "submit_proposal_batch",
                "semantics": "sequence",
                "proposals": [{
                    "effect": {
                        "kind": "change_from_agent_session",
                        "target": "cand_agent_kit_checkpoint_parent",
                        "agent_receipt": "agentrec_agent_kit_checkpoint",
                        "parser": "leaven.agent_session.skill_patch.v1",
                        "surface_fingerprint": "fp_surface_sha256_agent_kit_checkpoint",
                        "change_schema": "fp_schema_sha256_agent_kit_checkpoint_change"
                    },
                    "causal": {"inputs": ["cand_agent_kit_checkpoint_parent"]},
                    "informed_by": {
                        "kind": "literal",
                        "value": ["qrec_agent_kit_checkpoint_parent", "agentrec_agent_kit_checkpoint"]
                    },
                    "read_receipts": ["qrec_agent_kit_checkpoint_parent", "agentrec_agent_kit_checkpoint"]
                }]
            }
        }],
        "return": ["proposal_batch"],
        "commit": {"kind": "graph_writes_atomic", "on_stale": "reject"}
    })
}

fn apply_request(batch_ref: &str) -> Value {
    json!({
        "schema_version": "leaven.plan.v1",
        "plan_id": "plan_agent_kit_checkpoint_apply",
        "consistency": {"kind": "latest_at_start"},
        "mode": {"kind": "execute"},
        "ops": [{
            "kind": "write",
            "name": "apply",
            "idempotency_key": "agent-kit-checkpoint-apply-0001",
            "write": {
                "kind": "apply_proposal_batch",
                "proposal_batch": batch_ref,
                "policy": "apply_first_valid"
            }
        }],
        "return": ["apply"],
        "commit": {"kind": "graph_writes_atomic", "on_stale": "reject"}
    })
}

fn jsonrpc_request(id: &str, method: &str, params: &Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": method,
        "params": params,
    })
}

fn write_workspace_file(view: &mut leaven_workspace::WorkspaceView<'_>, path: &str, body: &str) {
    view.write_file(&workspace_path(path), body.as_bytes())
        .unwrap();
}

fn workspace_git<const N: usize>(
    view: &mut leaven_workspace::WorkspaceView<'_>,
    cwd: &str,
    args: [&str; N],
) -> Result<(), OptimizerError> {
    let mut command = Command::new("git");
    command.cwd = Some(workspace_path(cwd));
    command.args = args.iter().map(|arg| (*arg).to_owned()).collect();
    let output = view
        .run_command(command)
        .map_err(|source| OptimizerError::with_source("run workspace git", source))?;
    if output.status.code != Some(0) {
        return Err(OptimizerError::Message(format!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr.bytes)
        )));
    }
    Ok(())
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

#[derive(Clone, Debug)]
struct AgentKitCheckpointProblem;

impl OptimizationProblem for AgentKitCheckpointProblem {
    type Artifact = GitProgramArtifact;
    type Case = AgentKitCase;
    type Evidence = AgentKitEvidence;
    type ProposalAnnotations = ();
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct AgentKitCase;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct AgentKitEvidence;

impl Evidence for AgentKitEvidence {}

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

fn workspace_path(path: &str) -> WorkspacePath {
    WorkspacePath::new(path).unwrap()
}

fn git_object(hex: &str) -> GitObjectId {
    GitObjectId::new(hex).unwrap()
}

fn workspace_root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("crate is under workspace/crates/leaven-gepa-agentic-agent-kit")
        .to_path_buf()
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
