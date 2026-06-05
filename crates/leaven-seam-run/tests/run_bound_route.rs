use std::error::Error;
use std::fmt;
use std::io::Cursor;

use leaven_core::{
    Artifact, ArtifactIdentity, Assessment, AssessmentGranularity, AssessmentTarget,
    EvaluationPurpose, EvaluationRequest, EvaluationSet, Evidence, OptimizationProblem, Proposal,
    ProposalBatch, ProposalBatchSemantics,
};
use leaven_engine::{
    CaseSet, Optimizer, OptimizerError, RunContext, RunEvent, StepStatus, StoreRunPersistence,
};
use leaven_kernel::{
    Budget, CandidateId, ContentId, Cost, EvaluatorId, Fingerprint, MetadataBag, Metered,
};
use leaven_seam_run::RunBoundSdkRoute;
use leaven_seam_service::{RunBoundEvaluationRequest, RunBoundGraphEffectService};
use leaven_store_file::FileStore;
use leaven_store_inline::InlineEvidenceStore;
use serde_json::{Value, json};
use tempfile::TempDir;

#[test]
fn run_bound_sdk_route_mounts_inside_engine_lifecycle_and_restores_graph_truth() {
    let temp = TempDir::new().unwrap();
    let store = FileStore::open(temp.path()).unwrap();
    let persistence = StoreRunPersistence::new(store);
    let evidence_store = InlineEvidenceStore::<RouteEvidence>::new("route");
    let case_set = CaseSet::new(vec![RouteCase]);
    let mut engine = leaven_engine::Engine::<RouteProblem>::builder()
        .budget(Budget::unlimited())
        .persistence(persistence.clone())
        .build();
    let seed = engine.insert_seed(RouteArtifact(1), 0).unwrap();
    let mut optimizer = RouteMountedOptimizer {
        seed,
        mounted: false,
    };

    futures::executor::block_on(engine.run(&mut optimizer, &case_set, &evidence_store)).unwrap();

    assert!(
        optimizer.mounted,
        "optimizer step should mount the SDK route"
    );
    let restored = persistence
        .latest_checkpoint::<RouteProblem>()
        .unwrap()
        .expect("engine should advance the latest checkpoint");
    let mut restored_graph = restored.graph;
    let mut restored_budget = restored.budget;
    let restored_ctx = RunContext::<RouteProblem>::new(&mut restored_graph, &mut restored_budget);
    let graph = restored_ctx.graph();
    assert_eq!(graph.candidate_count(), 2);
    assert_eq!(graph.evaluation_request_count(), 1);
    assert_eq!(graph.assessment_count(), 1);
    assert!(graph.events().any(|event| matches!(
        event,
        RunEvent::ExternalEventEmitted { event_kind, .. }
            if event_kind == "route.checked"
    )));
}

#[test]
fn run_bound_sdk_route_refuses_configured_service_alias_batches() {
    let evidence_store = InlineEvidenceStore::<RouteEvidence>::new("route");
    let mut graph = leaven_engine::RunGraph::new(leaven_kernel::RunId::new());
    let mut budget = leaven_engine::BudgetLedger::new(Budget::unlimited());
    let mut ctx = RunContext::<RouteProblem>::new(&mut graph, &mut budget)
        .with_evidence_store(&evidence_store);
    let service = RunBoundGraphEffectService::new(
        &mut ctx,
        [],
        "fp_cap_sha256_route",
        "fp_policy_sha256_route",
        "rev_route_base",
        "rev_route_final",
    );
    let route = RunBoundSdkRoute::bind_run_bound_service(workspace_root(), service).unwrap();

    let responses = serve_jsonrpc_lines(
        &route,
        [jsonrpc_request(
            "route-reject-configured-alias",
            "leaven/proposal.apply",
            apply_request("pb_configured_run_context"),
        )],
    )
    .unwrap();

    let response = &responses[0];
    assert_eq!(
        response["id"],
        json!("route-reject-configured-alias"),
        "route must preserve request identity on refusal"
    );
    assert!(
        response["error"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("proposal_batch must be a pb_<uuid> ref")),
        "route should refuse configured-service alias batch refs: {response:?}"
    );
}

struct RouteMountedOptimizer {
    seed: CandidateId,
    mounted: bool,
}

impl Optimizer<RouteProblem> for RouteMountedOptimizer {
    async fn step(
        &mut self,
        ctx: &mut RunContext<'_, RouteProblem>,
    ) -> Result<StepStatus, OptimizerError> {
        let seed = self.seed;
        let service = RunBoundGraphEffectService::new(
            ctx,
            [],
            "fp_cap_sha256_route",
            "fp_policy_sha256_route",
            "rev_route_base",
            "rev_route_final",
        )
        .with_proposal_submitter({
            move |params| {
                if params.plan_id() != "plan_route_submit" {
                    return Err(format!(
                        "unexpected proposal submit plan {}",
                        params.plan_id()
                    ));
                }
                if params.proposals_payload()[0]["effect"]["kind"] != "change_from_agent_session" {
                    return Err("unexpected route proposal effect".to_owned());
                }
                Ok(ProposalBatch {
                    proposals: vec![Proposal::mutate(seed, 17).build()],
                    semantics: ProposalBatchSemantics::Alternatives,
                    metadata: MetadataBag::new(),
                })
            }
        })
        .with_evaluation_requester({
            move |_params| {
                Ok(RunBoundEvaluationRequest {
                    evaluator: EvaluatorId::from("eval_route"),
                    evaluator_fingerprint: Fingerprint::from_bytes([41; 32]),
                    request: EvaluationRequest::Independent {
                        candidates: vec![seed],
                        set: EvaluationSet::All,
                        granularity: AssessmentGranularity::PerCase,
                        purpose: EvaluationPurpose::Validation,
                    },
                })
            }
        })
        .with_assessment_submitter({
            move |_params| {
                Ok(Metered::new(
                    vec![Assessment::Independent {
                        candidate: seed,
                        target: AssessmentTarget::Unscoped,
                        evidence: RouteEvidence,
                        cost: Cost::zero(),
                        metadata: MetadataBag::new(),
                    }],
                    Cost::zero(),
                ))
            }
        });
        let route = RunBoundSdkRoute::bind_run_bound_service(workspace_root(), service)
            .map_err(|source| OptimizerError::with_source("bind run-bound SDK route", source))?;
        assert!(
            route
                .methods()
                .any(|method| method == "leaven/proposal.apply")
        );

        let submit = serve_jsonrpc_lines(
            &route,
            [jsonrpc_request(
                "route-submit",
                "leaven/proposal.submit_batch",
                submit_request(),
            )],
        )?;
        assert_success(&submit[0], "leaven/proposal.submit_batch")?;
        let batch_ref = submit[0]["result"]["primary"]["batch_id"]
            .as_str()
            .ok_or_else(|| {
                OptimizerError::Message("route submit response missing batch id".to_owned())
            })?
            .to_owned();
        let first = serve_jsonrpc_lines(
            &route,
            [
                jsonrpc_request(
                    "route-apply",
                    "leaven/proposal.apply",
                    apply_request(&batch_ref),
                ),
                jsonrpc_request(
                    "route-evaluation",
                    "leaven/evaluation.request",
                    evaluation_request(),
                ),
            ],
        )?;
        assert_success(&first[0], "leaven/proposal.apply")?;
        assert_success(&first[1], "leaven/evaluation.request")?;
        let evaluation_ref = first[1]["result"]["primary"]["evaluation_request_id"]
            .as_str()
            .ok_or_else(|| {
                OptimizerError::Message("route evaluation response missing request id".to_owned())
            })?
            .to_owned();
        let second = serve_jsonrpc_lines(
            &route,
            [
                jsonrpc_request(
                    "route-assessment",
                    "leaven/assessment.submit",
                    assessment_request(&evaluation_ref),
                ),
                jsonrpc_request("route-event", "leaven/event.emit", event_request()),
            ],
        )?;
        assert_success(&second[0], "leaven/assessment.submit")?;
        assert_success(&second[1], "leaven/event.emit")?;
        self.mounted = true;
        Ok(StepStatus::Done)
    }

    fn best_candidate(
        &self,
        graph: leaven_engine::RunGraphView<'_, RouteProblem>,
    ) -> Option<CandidateId> {
        graph
            .candidate_tree()
            .roots()
            .last()
            .copied()
            .or(Some(self.seed))
    }
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
    route: &RunBoundSdkRoute<RunBoundGraphEffectService<'_, '_, RouteProblem>>,
    requests: [Value; N],
) -> Result<Vec<Value>, OptimizerError> {
    let input = requests
        .into_iter()
        .map(|request| serde_json::to_string(&request))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|source| OptimizerError::with_source("serialize route request", source))?
        .join("\n");
    let mut output = Vec::new();
    route
        .serve_reader_writer(Cursor::new(format!("{input}\n")), &mut output)
        .map_err(|source| OptimizerError::with_source("serve run-bound SDK route", source))?;
    Ok(String::from_utf8(output)
        .map_err(|source| OptimizerError::with_source("decode route output", source))?
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|source| OptimizerError::with_source("parse route response", source))?)
}

fn jsonrpc_request(id: &str, method: &str, params: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "method": method,
        "params": params,
    })
}

fn submit_request() -> Value {
    json!({
        "schema_version": "leaven.plan.v1",
        "plan_id": "plan_route_submit",
        "consistency": {"kind": "latest_at_start"},
        "mode": {"kind": "execute"},
        "ops": [{
            "kind": "write",
            "name": "proposal_batch",
            "idempotency_key": "route-submit-0001",
            "write": {
                "kind": "submit_proposal_batch",
                "semantics": "sequence",
                "proposals": [{
                    "effect": {
                        "kind": "change_from_agent_session",
                        "target": "cand_route_parent",
                        "agent_receipt": "agentrec_route",
                        "parser": "leaven.agent_session.route_patch.v1",
                        "surface_fingerprint": "fp_surface_sha256_route",
                        "change_schema": "fp_schema_sha256_route_change"
                    },
                    "causal": {"inputs": ["cand_route_parent"]},
                    "informed_by": {
                        "kind": "literal",
                        "value": ["qrec_route_parent", "agentrec_route"]
                    },
                    "read_receipts": ["qrec_route_parent", "agentrec_route"]
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
        "plan_id": "plan_route_apply",
        "consistency": {"kind": "latest_at_start"},
        "mode": {"kind": "execute"},
        "ops": [{
            "kind": "write",
            "name": "apply",
            "idempotency_key": "route-apply-0001",
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

fn evaluation_request() -> Value {
    json!({
        "schema_version": "leaven.plan.v1",
        "plan_id": "plan_route_evaluation",
        "consistency": {"kind": "latest_at_start"},
        "mode": {"kind": "execute"},
        "ops": [{
            "kind": "write",
            "name": "evaluation_request",
            "idempotency_key": "route-evaluation-0001",
            "write": {
                "kind": "request_evaluation",
                "request": {
                    "evaluator": "eval_route",
                    "shape": "independent",
                    "candidates": ["cand_placeholder"],
                    "set": {"kind": "named", "name": "validation"},
                    "granularity": "per_case",
                    "purpose": "validation"
                }
            }
        }],
        "return": ["evaluation_request"],
        "commit": {"kind": "graph_writes_atomic", "on_stale": "reject"}
    })
}

fn assessment_request(evaluation_request_id: &str) -> Value {
    json!({
        "schema_version": "leaven.plan.v1",
        "plan_id": "plan_route_assessment",
        "consistency": {"kind": "latest_at_start"},
        "mode": {"kind": "execute"},
        "ops": [{
            "kind": "write",
            "name": "assessment_batch",
            "idempotency_key": "route-assessment-0001",
            "write": {
                "kind": "submit_assessments",
                "evaluation_request_id": evaluation_request_id,
                "assessments": [{
                    "kind": "independent",
                    "candidate": "cand_route",
                    "target": {"case": "case_route"},
                    "score": {
                        "value": 1.0,
                        "output": {
                            "kind": "structured",
                            "summary": "route child assessed",
                            "value": {"candidate": "cand_route", "output": "route child assessed"},
                            "visibility": "public",
                            "data_classes": ["candidate.output"]
                        }
                    },
                    "evidence": {
                        "schema_version": "leaven.evidence_envelope.v1",
                        "target_derived": false,
                        "public": {
                            "summary": "route child assessed",
                            "data_classes": ["public"]
                        },
                        "redaction_policy": {
                            "optimizer": "score_only",
                            "reflector": "score_only",
                            "operator": "score_only"
                        },
                        "producer": {"stage_call_id": "sc_route_assessment"},
                        "source_receipts": {
                            "read": ["qrec_route_assessment_source"],
                            "effect": []
                        }
                    },
                    "replayability": "pure_read"
                }]
            }
        }],
        "return": ["assessment_batch"],
        "commit": {"kind": "graph_writes_atomic", "on_stale": "reject"}
    })
}

fn event_request() -> Value {
    json!({
        "schema_version": "leaven.plan.v1",
        "plan_id": "plan_route_event",
        "consistency": {"kind": "latest_at_start"},
        "mode": {"kind": "execute"},
        "ops": [{
            "kind": "write",
            "name": "route_event",
            "idempotency_key": "route-event-0001",
            "write": {
                "kind": "emit_run_event",
                "event_kind": "route.checked",
                "payload_schema": "fp_schema_sha256_route_test",
                "payload": {"ok": true},
                "visibility": "public"
            }
        }],
        "return": ["route_event"],
        "commit": {"kind": "graph_writes_atomic", "on_stale": "reject"}
    })
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct RouteArtifact(i32);

impl Artifact for RouteArtifact {
    type Change = i32;
    type ApplyError = RouteApplyError;

    fn identity(&self) -> ArtifactIdentity {
        ArtifactIdentity::Content(ContentId::hash_bytes(self.0.to_le_bytes()))
    }

    fn apply_change(&self, change: &Self::Change) -> Result<Self, Self::ApplyError> {
        Ok(Self(self.0 + change))
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct RouteEvidence;

impl Evidence for RouteEvidence {}

#[derive(Clone, Debug)]
struct RouteCase;

struct RouteProblem;

impl OptimizationProblem for RouteProblem {
    type Artifact = RouteArtifact;
    type Case = RouteCase;
    type Evidence = RouteEvidence;
    type ProposalAnnotations = ();
}

#[derive(Debug)]
struct RouteApplyError;

impl fmt::Display for RouteApplyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("route apply failed")
    }
}

impl Error for RouteApplyError {}

fn workspace_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("crate is under workspace/crates/leaven-seam-run")
        .to_path_buf()
}
