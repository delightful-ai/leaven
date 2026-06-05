use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::time::Duration;

use bytes::Bytes;
use leaven_agent::{
    AgentContextRef, AgentInstructions, AgentLimits, AgentRunContext, AgentRunRequest,
    AgentRuntime, OutputContract,
};
use leaven_agent_codex_cli::{CodexCliConfig, CodexCliRuntime};
use leaven_agentic_agent_kit::{AgentKitMountMode, CodexAgentKitMaterializer};
use leaven_agentic_git::{GitProgramMaterializer, GitProgramReadback, GitProgramStores};
use leaven_artifact_git::GitProgramArtifact;
use leaven_core::{InfoRef, Proposal, ProposalBatch, ProposalBatchSemantics};
use leaven_engine::{
    CaseSet, Optimizer, OptimizerError, RunContext, StepStatus, StoreRunPersistence,
};
use leaven_kernel::{AgentSessionId, Budget, BudgetSnapshot, CandidateId, MetadataBag};
use leaven_seam_run::RunBoundSdkRoute;
use leaven_seam_service::RunBoundGraphEffectService;
use leaven_store::{BlobStore, BlobWrite};
use leaven_store_file::FileStore;
use leaven_store_inline::InlineEvidenceStore;
use leaven_workspace::{WorkspaceFactory, WorkspacePath};
use leaven_workspace_local::LocalWorkspaceFactory;
use serde_json::{Value, json};

use crate::live_codex_agent_kit_fixture::{
    LiveAgentKitCase, LiveAgentKitEvidence, LiveAgentKitProblem, LiveAgentKitRepoFixture,
};

pub async fn run_live_codex_agentkit_evolution() {
    let run_dir = tempfile::tempdir().unwrap();
    let store = FileStore::open(run_dir.path()).unwrap();
    let persistence = StoreRunPersistence::new(store.clone());
    let fixture = LiveAgentKitRepoFixture::new();
    let parent_artifact = fixture.program_artifact();
    let stores = fixture.stores();
    let mut engine = leaven_engine::Engine::<LiveAgentKitProblem>::builder()
        .budget(Budget::unlimited())
        .persistence(persistence.clone())
        .build();
    let seed = engine.insert_seed(parent_artifact.clone(), 0).unwrap();
    let evidence_store = InlineEvidenceStore::<LiveAgentKitEvidence>::new("live-agent-kit");
    let cases = CaseSet::new(Vec::<LiveAgentKitCase>::new());
    let mut optimizer = LiveAgentKitOptimizer {
        seed,
        parent_artifact,
        stores: stores.clone(),
        store: store.clone(),
        mounted: false,
    };

    engine
        .run(&mut optimizer, &cases, &evidence_store)
        .await
        .unwrap();
    assert!(
        optimizer.mounted,
        "live optimizer must submit/apply over SDK route"
    );

    let export = leaven_run::export_local_run_inspection(run_dir.path()).unwrap();
    assert_eq!(export.graph.candidate_count, 2);
    assert_eq!(export.graph.proposal_count, 1);
    assert_eq!(export.graph.apply_attempt_count, 1);
    assert_eq!(export.checkpoint.stage_journal_ref_count, 1);
    let transcript_ref = &export.checkpoint.stage_journal_refs[0];
    let transcript = leaven_run::export_local_run_blob(
        run_dir.path(),
        &transcript_ref.store,
        &transcript_ref.key,
    )
    .unwrap();
    assert_eq!(transcript.content_base64, LIVE_AGENT_KIT_TRANSCRIPT_BASE64);

    let restored = persistence
        .latest_checkpoint::<LiveAgentKitProblem>()
        .unwrap()
        .expect("live AgentKit run writes a checkpoint");
    let mut restored_graph = restored.graph;
    let mut restored_budget = restored.budget;
    let restored_ctx =
        RunContext::<LiveAgentKitProblem>::new(&mut restored_graph, &mut restored_budget);
    let child = restored_ctx.graph().children(seed)[0];
    let child_artifact = restored_ctx.graph().artifact(child).unwrap().clone();

    let consumed = run_live_codex_child_consumption(&child_artifact, stores).await;
    assert_eq!(consumed.system_proof, "CHILD_SYSTEM_CONSUMED\n");
    assert_eq!(consumed.skill_proof, "CHILD_SKILL_CONSUMED\n");
}

struct LiveAgentKitOptimizer {
    seed: CandidateId,
    parent_artifact: GitProgramArtifact,
    stores: GitProgramStores,
    store: FileStore,
    mounted: bool,
}

impl Optimizer<LiveAgentKitProblem> for LiveAgentKitOptimizer {
    async fn step(
        &mut self,
        ctx: &mut RunContext<'_, LiveAgentKitProblem>,
    ) -> Result<StepStatus, OptimizerError> {
        let change =
            read_back_live_codex_agent_kit_change(&self.parent_artifact, self.stores.clone())
                .await?;
        let transcript_ref = BlobStore::put(
            &self.store,
            BlobWrite {
                bytes: Bytes::from_static(LIVE_AGENT_KIT_TRANSCRIPT_BYTES),
                content_type: Some("application/json".to_owned()),
            },
        )
        .map_err(|source| OptimizerError::with_source("write live AgentKit transcript", source))?;
        ctx.record_stage_journal_entry(transcript_ref)
            .map_err(|source| {
                OptimizerError::with_source("record live AgentKit transcript", source)
            })?;
        let seed = self.seed;
        let service = RunBoundGraphEffectService::new(
            ctx,
            [],
            "fp_cap_sha256_live_agent_kit",
            "fp_policy_sha256_live_agent_kit",
            "rev_live_agent_kit_base",
            "rev_live_agent_kit_child",
        )
        .with_proposal_submitter({
            move |params| {
                if params.plan_id() != "plan_live_agent_kit_submit" {
                    return Err(format!(
                        "unexpected live AgentKit plan {}",
                        params.plan_id()
                    ));
                }
                if params.proposals_payload()[0]["effect"]["kind"] != "change_from_agent_session" {
                    return Err("unexpected live AgentKit proposal effect".to_owned());
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
        let route = RunBoundSdkRoute::bind_run_bound_service(workspace_root(), service).map_err(
            |source| OptimizerError::with_source("bind live AgentKit SDK route", source),
        )?;
        let submit = serve_jsonrpc_lines(
            &route,
            [jsonrpc_request(
                "live-agent-kit-submit",
                "leaven/proposal.submit_batch",
                live_submit_request(),
            )],
        )?;
        assert_success(&submit[0], "leaven/proposal.submit_batch")?;
        let batch_ref = submit[0]["result"]["primary"]["batch_id"]
            .as_str()
            .ok_or_else(|| {
                OptimizerError::Message("live AgentKit submit response missing batch id".to_owned())
            })?
            .to_owned();
        let apply = serve_jsonrpc_lines(
            &route,
            [jsonrpc_request(
                "live-agent-kit-apply",
                "leaven/proposal.apply",
                live_apply_request(&batch_ref),
            )],
        )?;
        assert_success(&apply[0], "leaven/proposal.apply")?;
        self.mounted = true;
        Ok(StepStatus::Done)
    }

    fn best_candidate(
        &self,
        graph: leaven_engine::RunGraphView<'_, LiveAgentKitProblem>,
    ) -> Option<CandidateId> {
        graph
            .children(self.seed)
            .first()
            .copied()
            .or(Some(self.seed))
    }
}

async fn read_back_live_codex_agent_kit_change(
    parent_artifact: &GitProgramArtifact,
    stores: GitProgramStores,
) -> Result<leaven_artifact_git::GitProgramChange, OptimizerError> {
    let mut workspace = LocalWorkspaceFactory::temp()
        .allocate(Default::default())
        .await
        .map_err(|source| {
            OptimizerError::with_source("allocate live AgentKit mutation workspace", source)
        })?;
    let change = {
        let mut view = workspace.view();
        GitProgramMaterializer::new(stores.clone())
            .materialize_program(parent_artifact, &mut view)
            .map_err(|source| {
                OptimizerError::with_source("materialize live AgentKit parent", source)
            })?;
        let root = view
            .local_mount()
            .expect("live proof needs local mount")
            .to_path_buf();
        let projection = CodexAgentKitMaterializer::new(AgentKitMountMode::Copy)
            .materialize(root.join("repos/agent"), &root)
            .map_err(|source| {
                OptimizerError::with_source("project live AgentKit into Codex ABI", source)
            })?;
        run_live_codex_reflection_stage(&mut view, &projection.system_prompt).await?;
        let mut instructions = AgentInstructions::task(
            "Read agent/reflection.md, then edit only files under repos/agent. Replace \
             repos/agent/system_prompt.md with exactly:\n\
             Child system proof requirement: create agent/child-system-proof.txt containing exactly CHILD_SYSTEM_CONSUMED\n\
             \n\
             Replace repos/agent/skills/alpha/SKILL.md with exactly:\n\
             ---\nname: alpha\ndescription: Use when asked for the child alpha proof.\n---\n\n\
             When this skill is invoked, create agent/child-skill-proof.txt containing exactly CHILD_SKILL_CONSUMED.\n\
             \n\
             Do not edit AGENTS.md, hooks, .agents, or files outside repos/agent.",
        );
        instructions.system = projection.system_prompt.clone();
        instructions.context.push(AgentContextRef {
            label: "reflection".to_owned(),
            path: WorkspacePath::new("agent/reflection.md").unwrap(),
            media_type: Some("text/markdown".to_owned()),
        });
        let mut request = AgentRunRequest::new(
            instructions,
            OutputContract::WorkspaceDiff {
                roots: vec![WorkspacePath::new("repos/agent").unwrap()],
                surface_fingerprint: None,
            },
        );
        request.limits = live_limits();
        CodexCliRuntime::new(codex_config())
            .run_session(
                &mut view,
                request,
                AgentRunContext::new(AgentSessionId::new(), &BudgetSnapshot::default()),
            )
            .await
            .map_err(|source| {
                OptimizerError::with_source("run live Codex AgentKit mutation stage", source)
            })?;
        assert_live_codex_authored_child_files(&root);
        GitProgramReadback::new(stores)
            .read_back_change(parent_artifact, &mut view)
            .map_err(|source| {
                OptimizerError::with_source("read back live Codex AgentKit change", source)
            })?
            .ok_or_else(|| {
                OptimizerError::Message(
                    "live Codex AgentKit mutation produced no Git change".to_owned(),
                )
            })?
    };
    workspace.cleanup().await.map_err(|source| {
        OptimizerError::with_source("cleanup live AgentKit mutation workspace", source)
    })?;
    Ok(change)
}

async fn run_live_codex_reflection_stage(
    view: &mut leaven_workspace::WorkspaceView<'_>,
    system_prompt: &Option<String>,
) -> Result<(), OptimizerError> {
    let mut instructions = AgentInstructions::task(
        "Inspect repos/agent/system_prompt.md and repos/agent/skills/alpha/SKILL.md. \
         Do not edit repos/agent. Create agent/reflection.md containing exactly:\n\
         REFLECTION_DIAGNOSIS: replace the parent prompt and alpha skill with child proof instructions.\n",
    );
    instructions.system.clone_from(system_prompt);
    let mut request = AgentRunRequest::new(
        instructions,
        OutputContract::Files {
            paths: vec![WorkspacePath::new("agent/reflection.md").unwrap()],
        },
    );
    request.limits = live_limits();
    CodexCliRuntime::new(codex_config())
        .run_session(
            view,
            request,
            AgentRunContext::new(AgentSessionId::new(), &BudgetSnapshot::default()),
        )
        .await
        .map_err(|source| {
            OptimizerError::with_source("run live Codex AgentKit reflection stage", source)
        })?;
    let reflection = view
        .read_file(&WorkspacePath::new("agent/reflection.md").unwrap())
        .map_err(|source| OptimizerError::with_source("read live Codex reflection", source))?;
    if reflection.as_slice()
        != b"REFLECTION_DIAGNOSIS: replace the parent prompt and alpha skill with child proof instructions.\n"
    {
        return Err(OptimizerError::Message(format!(
            "live Codex reflection body mismatch: {}",
            String::from_utf8_lossy(&reflection)
        )));
    }
    Ok(())
}

async fn run_live_codex_child_consumption(
    child_artifact: &GitProgramArtifact,
    stores: GitProgramStores,
) -> LiveChildConsumption {
    let mut workspace = LocalWorkspaceFactory::temp()
        .allocate(Default::default())
        .await
        .unwrap();
    let consumed = {
        let mut view = workspace.view();
        GitProgramMaterializer::new(stores)
            .materialize_program(child_artifact, &mut view)
            .unwrap();
        let root = view.local_mount().unwrap().to_path_buf();
        let projection = CodexAgentKitMaterializer::new(AgentKitMountMode::Copy)
            .materialize(root.join("repos/agent"), &root)
            .unwrap();
        let mut instructions = AgentInstructions::task(
            "Read the active system prompt and the projected alpha skill at \
             .agents/skills/alpha/SKILL.md. Use only those two projected AgentKit \
             sources to determine the required exact file contents, then create \
             agent/child-system-proof.txt and agent/child-skill-proof.txt.",
        );
        instructions.system = projection.system_prompt;
        instructions.context.push(AgentContextRef {
            label: "alpha-skill".to_owned(),
            path: WorkspacePath::new(".agents/skills/alpha/SKILL.md").unwrap(),
            media_type: Some("text/markdown".to_owned()),
        });
        let mut request = AgentRunRequest::new(
            instructions,
            OutputContract::Files {
                paths: vec![
                    WorkspacePath::new("agent/child-system-proof.txt").unwrap(),
                    WorkspacePath::new("agent/child-skill-proof.txt").unwrap(),
                ],
            },
        );
        request.limits = live_limits();
        if let Err(source) = CodexCliRuntime::new(codex_config())
            .run_session(
                &mut view,
                request,
                AgentRunContext::new(AgentSessionId::new(), &BudgetSnapshot::default()),
            )
            .await
        {
            panic!(
                "run live Codex AgentKit child-consumption stage: {source:?}\n{}",
                child_consumption_debug(&root)
            );
        }
        LiveChildConsumption {
            system_proof: fs::read_to_string(root.join("agent/child-system-proof.txt")).unwrap(),
            skill_proof: fs::read_to_string(root.join("agent/child-skill-proof.txt")).unwrap(),
        }
    };
    workspace.cleanup().await.unwrap();
    consumed
}

struct LiveChildConsumption {
    system_proof: String,
    skill_proof: String,
}

fn child_consumption_debug(root: &Path) -> String {
    format!(
        "workspace: {}\nroot files: {:?}\nagent files: {:?}\nprojected skill: {:?}\nlast message: {:?}",
        root.display(),
        sorted_names(root),
        sorted_names(&root.join("agent")),
        fs::read_to_string(root.join(".agents/skills/alpha/SKILL.md")).ok(),
        fs::read_to_string(root.join(".leaven/codex-last-message.txt")).ok()
    )
}

fn sorted_names(path: &Path) -> Vec<String> {
    let Ok(entries) = fs::read_dir(path) else {
        return Vec::new();
    };
    let mut names = entries
        .filter_map(Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    names.sort();
    names
}

fn assert_live_codex_authored_child_files(root: &Path) {
    assert_eq!(
        fs::read_to_string(root.join("repos/agent/system_prompt.md")).unwrap(),
        "Child system proof requirement: create agent/child-system-proof.txt containing exactly CHILD_SYSTEM_CONSUMED\n"
    );
    assert_eq!(
        fs::read_to_string(root.join("repos/agent/skills/alpha/SKILL.md")).unwrap(),
        "---\nname: alpha\ndescription: Use when asked for the child alpha proof.\n---\n\nWhen this skill is invoked, create agent/child-skill-proof.txt containing exactly CHILD_SKILL_CONSUMED.\n"
    );
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
    route: &RunBoundSdkRoute<RunBoundGraphEffectService<'_, '_, LiveAgentKitProblem>>,
    requests: [Value; N],
) -> Result<Vec<Value>, OptimizerError> {
    let input = requests
        .into_iter()
        .map(|request| serde_json::to_string(&request))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|source| {
            OptimizerError::with_source("serialize live AgentKit route request", source)
        })?
        .join("\n");
    let mut output = Vec::new();
    route
        .serve_reader_writer(Cursor::new(format!("{input}\n")), &mut output)
        .map_err(|source| OptimizerError::with_source("serve live AgentKit route", source))?;
    Ok(String::from_utf8(output)
        .map_err(|source| OptimizerError::with_source("decode live AgentKit route output", source))?
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|source| {
            OptimizerError::with_source("parse live AgentKit route response", source)
        })?)
}

fn jsonrpc_request(id: &str, method: &str, params: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": method,
        "params": params,
    })
}

fn live_submit_request() -> Value {
    json!({
        "schema_version": "leaven.plan.v1",
        "plan_id": "plan_live_agent_kit_submit",
        "consistency": {"kind": "latest_at_start"},
        "mode": {"kind": "execute"},
        "ops": [{
            "kind": "write",
            "name": "proposal_batch",
            "idempotency_key": "live-agent-kit-submit-0001",
            "write": {
                "kind": "submit_proposal_batch",
                "semantics": "sequence",
                "proposals": [{
                    "effect": {
                        "kind": "change_from_agent_session",
                        "target": "cand_live_agent_kit_parent",
                        "agent_receipt": "agentrec_live_codex",
                        "parser": "leaven.agent_session.skill_patch.v1",
                        "surface_fingerprint": "fp_surface_sha256_live_agent_kit",
                        "change_schema": "fp_schema_sha256_live_agent_kit_change"
                    },
                    "causal": {"inputs": ["cand_live_agent_kit_parent"]},
                    "informed_by": {
                        "kind": "literal",
                        "value": ["qrec_live_agent_kit_parent", "agentrec_live_codex"]
                    },
                    "read_receipts": ["qrec_live_agent_kit_parent", "agentrec_live_codex"]
                }]
            }
        }],
        "return": ["proposal_batch"],
        "commit": {"kind": "graph_writes_atomic", "on_stale": "reject"}
    })
}

fn live_apply_request(batch_ref: &str) -> Value {
    json!({
        "schema_version": "leaven.plan.v1",
        "plan_id": "plan_live_agent_kit_apply",
        "consistency": {"kind": "latest_at_start"},
        "mode": {"kind": "execute"},
        "ops": [{
            "kind": "write",
            "name": "apply",
            "idempotency_key": "live-agent-kit-apply-0001",
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

fn codex_config() -> CodexCliConfig {
    let mut config = CodexCliConfig::new(codex_bin());
    config.timeout = Some(Duration::from_secs(180));
    config
}

fn codex_bin() -> String {
    if let Some(path) = std::env::var_os("LEAVEN_CODEX_BIN") {
        return path.to_string_lossy().into_owned();
    }
    let home = std::env::var_os("HOME").expect("HOME must be set for Codex live test");
    PathBuf::from(home)
        .join(".bun/bin/codex")
        .to_string_lossy()
        .into_owned()
}

fn live_limits() -> AgentLimits {
    AgentLimits {
        timeout: Some(Duration::from_secs(180)),
        ..AgentLimits::default()
    }
}

const LIVE_AGENT_KIT_TRANSCRIPT_BYTES: &[u8] =
    br#"{"kind":"live_agent_kit_e2e","stages":["live_codex_reflection","live_codex_mutation","stdio_submit_apply","checkpoint_restore","live_codex_child_consumption"]}"#;

const LIVE_AGENT_KIT_TRANSCRIPT_BASE64: &str = "eyJraW5kIjoibGl2ZV9hZ2VudF9raXRfZTJlIiwic3RhZ2VzIjpbImxpdmVfY29kZXhfcmVmbGVjdGlvbiIsImxpdmVfY29kZXhfbXV0YXRpb24iLCJzdGRpb19zdWJtaXRfYXBwbHkiLCJjaGVja3BvaW50X3Jlc3RvcmUiLCJsaXZlX2NvZGV4X2NoaWxkX2NvbnN1bXB0aW9uIl19";

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("crate is under workspace/crates/leaven-gepa-agentic-agent-kit")
        .to_path_buf()
}
