use std::error::Error;
use std::fmt;
use std::io::Cursor;

use leaven_core::{
    Artifact, ArtifactIdentity, Assessment, AssessmentGranularity, AssessmentTarget,
    EvaluationPurpose, EvaluationRequest, EvaluationSet, Evidence, OptimizationProblem, Proposal,
    ProposalBatch, ProposalBatchSemantics,
};
use leaven_engine::{
    BudgetLedger, CaseSet, Optimizer, OptimizerError, OptimizerStateWrite, RunContext, RunEvent,
    StepStatus, StoreRunPersistence,
};
use leaven_kernel::{
    Budget, CandidateId, ContentId, Cost, EvaluatorId, Fingerprint, MetadataBag, Metered, StageId,
};
use leaven_public_seam::LockedMethod;
use leaven_seam_runtime::SeamRuntime;
use leaven_seam_stdio::serve_reader_writer;
use leaven_store::CheckpointStore;
use leaven_store_file::FileStore;
use leaven_store_inline::InlineEvidenceStore;
use serde_json::{Value, json};
use tempfile::TempDir;

use super::*;

#[test]
fn run_bound_service_mutates_real_context_and_checkpoint_readback_sees_graph_truth() {
    let package = leaven_public_seam::PublicSeamPackage::active_from_repo(workspace_root())
        .expect("public seam package loads from workspace");
    let temp = TempDir::new().unwrap();
    let store = FileStore::open(temp.path()).unwrap();
    let persistence = StoreRunPersistence::new(store.clone());
    let evidence_store = InlineEvidenceStore::<RunBoundEvidence>::new("run-bound");
    let case_set = CaseSet::new(vec![RunBoundCase]);
    let mut graph = leaven_engine::RunGraph::new(leaven_kernel::RunId::new());
    let mut budget = BudgetLedger::new(Budget::unlimited());
    let mut ctx = RunContext::<RunBoundProblem>::new(&mut graph, &mut budget)
        .with_case_set(&case_set)
        .with_evidence_store(&evidence_store)
        .with_persistence(Some(&persistence));
    let seed = ctx.insert_seed(RunBoundArtifact(1), 0).unwrap();
    let batch = ctx
        .record_proposal_batch(
            StageId::custom("run-bound-proposer"),
            ProposalBatch {
                proposals: vec![Proposal::mutate(seed, 41).build()],
                semantics: ProposalBatchSemantics::Alternatives,
                metadata: MetadataBag::new(),
            },
            Cost::zero(),
        )
        .unwrap();
    let batch_ref = format!("pb_{}", batch.batch_id.as_uuid());
    let latest_evaluation: String;
    {
        let service = RunBoundGraphEffectService::new(
            &mut ctx,
            [batch],
            "fp_cap_sha256_run_bound",
            "fp_policy_sha256_run_bound",
            "rev_run_bound_base",
            "rev_run_bound_final",
        )
        .with_evaluation_requester({
            move |params| {
                assert_eq!(params.plan_id(), "plan_run_bound_evaluation");
                assert_eq!(params.op_name(), "evaluation_request");
                assert_eq!(params.request_payload()["shape"], "independent");
                Ok(RunBoundEvaluationRequest {
                    evaluator: EvaluatorId::from("eval_run_bound"),
                    evaluator_fingerprint: Fingerprint::from_bytes([37; 32]),
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
            move |params| {
                assert_eq!(params.plan_id(), "plan_run_bound_assessment");
                assert_eq!(params.op_name(), "assessment_batch");
                assert_eq!(params.assessments_payload()[0]["kind"], "independent");
                Ok(Metered::new(
                    vec![Assessment::Independent {
                        candidate: seed,
                        target: AssessmentTarget::Unscoped,
                        evidence: RunBoundEvidence,
                        cost: Cost::zero(),
                        metadata: MetadataBag::new(),
                    }],
                    Cost::zero(),
                ))
            }
        });

        let apply = service
            .handle_method(
                LockedMethod::ProposalApply,
                &proposal_apply_request(&batch_ref),
            )
            .unwrap();
        assert_eq!(apply["primary"]["kind"], "apply_receipt");
        assert_eq!(apply["receipts"][0]["write_kind"], "apply_proposal_batch");
        package
            .validate_acp_extension_result_document(&apply)
            .expect("apply result validates through public seam owner");

        let evaluation = service
            .handle_method(
                LockedMethod::EvaluationRequest,
                &evaluation_request_request(),
            )
            .unwrap();
        let evaluation_ref = evaluation["primary"]["evaluation_request_id"]
            .as_str()
            .unwrap()
            .to_owned();
        latest_evaluation = evaluation_ref.clone();
        assert_eq!(evaluation["primary"]["kind"], "evaluation_request_receipt");
        package
            .validate_acp_extension_result_document(&evaluation)
            .expect("evaluation request result validates through public seam owner");

        let assessment = service
            .handle_method(
                LockedMethod::AssessmentSubmit,
                &assessment_submit_request(&evaluation_ref),
            )
            .unwrap();
        assert_eq!(assessment["primary"]["kind"], "assessment_batch_receipt");
        assert_eq!(
            assessment["receipts"][0]["write_kind"],
            "submit_assessments"
        );
        package
            .validate_acp_extension_result_document(&assessment)
            .expect("assessment result validates through public seam owner");

        let event = service
            .handle_method(LockedMethod::EventEmit, &event_emit_request())
            .unwrap();
        assert_eq!(event["primary"]["kind"], "emit_run_event");
        assert_eq!(event["receipts"][0]["write_kind"], "emit_run_event");
        assert_eq!(event["primary"]["receipt"], "wrec_run_bound_event");
        assert_eq!(
            event["receipts"][0]["request_hash"],
            prefixed_jcs_hash(
                "fp_request_sha256_",
                &json!({
                    "schema_version": "leaven.plan_write_request.v1",
                    "name": "run_bound_event",
                    "kind": "emit_run_event",
                    "write": {
                        "kind": "emit_run_event",
                        "event_kind": "run_bound.checked",
                        "payload_schema": "fp_schema_sha256_run_bound_test",
                        "payload": {"ok": true},
                        "visibility": "public"
                    },
                    "deps": {},
                    "dependency_data_classes": [],
                    "base_revision": "rev_run_bound_base"
                })
            )
        );
        assert_eq!(
            event["receipts"][0]["result_hash"],
            prefixed_jcs_hash(
                "fp_result_sha256_",
                &json!({
                    "schema_version": "leaven.plan_write_result.v1",
                    "name": "run_bound_event",
                    "value": event["primary"]
                })
            )
        );
        package
            .validate_acp_extension_result_document(&event)
            .expect("event result validates through public seam owner");
    }

    ctx.checkpoint_with_optimizer_state(
        OptimizerStateWrite::json(
            Fingerprint::from_bytes([91; 32]),
            Fingerprint::from_bytes([92; 32]),
            &json!({"boundary": "run-bound-service"}),
        )
        .unwrap(),
    )
    .unwrap();
    drop(ctx);

    assert!(
        CheckpointStore::latest(&store).unwrap().is_some(),
        "run-bound service proof must advance a latest checkpoint for readback"
    );
    let restored = persistence
        .latest_checkpoint::<RunBoundProblem>()
        .unwrap()
        .expect("latest checkpoint restores");
    let mut restored_graph = restored.graph;
    let mut restored_budget = restored.budget;
    let restored_ctx =
        RunContext::<RunBoundProblem>::new(&mut restored_graph, &mut restored_budget);
    let graph = restored_ctx.graph();
    assert_eq!(graph.candidate_count(), 2);
    assert_eq!(graph.evaluation_request_count(), 1);
    assert_eq!(graph.assessment_count(), 1);
    assert!(
        graph.events().any(|event| matches!(
            event,
            RunEvent::ExternalEventEmitted { event_kind, .. }
                if event_kind == "run_bound.checked"
        )),
        "checkpoint graph should restore emitted public-seam event"
    );
    assert!(latest_evaluation.starts_with("evalreq_"));
}

#[test]
fn run_bound_service_refuses_configured_alias_batch_refs() {
    let evidence_store = InlineEvidenceStore::<RunBoundEvidence>::new("run-bound");
    let mut graph = leaven_engine::RunGraph::new(leaven_kernel::RunId::new());
    let mut budget = BudgetLedger::new(Budget::unlimited());
    let mut ctx = RunContext::<RunBoundProblem>::new(&mut graph, &mut budget)
        .with_evidence_store(&evidence_store);
    let service = RunBoundGraphEffectService::new(
        &mut ctx,
        [],
        "fp_cap_sha256_run_bound",
        "fp_policy_sha256_run_bound",
        "rev_run_bound_base",
        "rev_run_bound_final",
    );

    let error = service
        .handle_method(
            LockedMethod::ProposalApply,
            &proposal_apply_request("pb_configured_run_context"),
        )
        .unwrap_err();

    assert!(matches!(
        error,
        RunBoundGraphEffectError::InvalidProposalBatchRef
    ));
}

#[test]
fn run_bound_service_routes_graph_writes_through_runtime_and_stdio() {
    let package = leaven_public_seam::PublicSeamPackage::active_from_repo(workspace_root())
        .expect("public seam package loads from workspace");
    let temp = TempDir::new().unwrap();
    let store = FileStore::open(temp.path()).unwrap();
    let persistence = StoreRunPersistence::new(store.clone());
    let evidence_store = InlineEvidenceStore::<RunBoundEvidence>::new("run-bound");
    let case_set = CaseSet::new(vec![RunBoundCase]);
    let mut graph = leaven_engine::RunGraph::new(leaven_kernel::RunId::new());
    let mut budget = BudgetLedger::new(Budget::unlimited());
    let mut ctx = RunContext::<RunBoundProblem>::new(&mut graph, &mut budget)
        .with_case_set(&case_set)
        .with_evidence_store(&evidence_store)
        .with_persistence(Some(&persistence));
    let seed = ctx.insert_seed(RunBoundArtifact(1), 0).unwrap();
    let batch = ctx
        .record_proposal_batch(
            StageId::custom("run-bound-runtime-proposer"),
            ProposalBatch {
                proposals: vec![Proposal::mutate(seed, 5).build()],
                semantics: ProposalBatchSemantics::Alternatives,
                metadata: MetadataBag::new(),
            },
            Cost::zero(),
        )
        .unwrap();
    let batch_ref = format!("pb_{}", batch.batch_id.as_uuid());
    {
        let service = RunBoundGraphEffectService::new(
            &mut ctx,
            [batch],
            "fp_cap_sha256_run_bound",
            "fp_policy_sha256_run_bound",
            "rev_run_bound_base",
            "rev_run_bound_final",
        )
        .with_evaluation_requester({
            move |_params| {
                Ok(RunBoundEvaluationRequest {
                    evaluator: EvaluatorId::from("eval_run_bound"),
                    evaluator_fingerprint: Fingerprint::from_bytes([37; 32]),
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
                        evidence: RunBoundEvidence,
                        cost: Cost::zero(),
                        metadata: MetadataBag::new(),
                    }],
                    Cost::zero(),
                ))
            }
        });
        let runtime = SeamRuntime::from_package(package, service).unwrap();

        let first_input = format!(
            "{}\n{}\n",
            jsonrpc_request(
                "runtime-apply",
                "leaven/proposal.apply",
                proposal_apply_request(&batch_ref)
            ),
            jsonrpc_request(
                "runtime-evaluation",
                "leaven/evaluation.request",
                evaluation_request_request()
            ),
        );
        let mut first_output = Vec::new();
        let first_report =
            serve_reader_writer(&runtime, Cursor::new(first_input), &mut first_output).unwrap();
        assert_eq!(first_report.requests, 2);
        let first_lines = response_lines(first_output);
        assert!(
            first_lines.iter().all(|line| line.get("error").is_none()),
            "runtime/stdio returned error responses: {first_lines:?}"
        );
        assert_eq!(
            first_lines[0]["result"]["receipts"][0]["write_kind"],
            "apply_proposal_batch"
        );
        assert_eq!(
            first_lines[1]["result"]["primary"]["kind"],
            "evaluation_request_receipt"
        );
        let evaluation_ref = first_lines[1]["result"]["primary"]["evaluation_request_id"]
            .as_str()
            .unwrap()
            .to_owned();

        let second_input = format!(
            "{}\n{}\n",
            jsonrpc_request(
                "runtime-assessment",
                "leaven/assessment.submit",
                assessment_submit_request(&evaluation_ref),
            ),
            jsonrpc_request("runtime-event", "leaven/event.emit", event_emit_request()),
        );
        let mut second_output = Vec::new();
        let second_report =
            serve_reader_writer(&runtime, Cursor::new(second_input), &mut second_output).unwrap();
        assert_eq!(second_report.requests, 2);
        let second_lines = response_lines(second_output);
        assert!(
            second_lines.iter().all(|line| line.get("error").is_none()),
            "runtime/stdio returned error responses: {second_lines:?}"
        );
        assert_eq!(
            second_lines[0]["result"]["receipts"][0]["write_kind"],
            "submit_assessments"
        );
        assert_eq!(
            second_lines[1]["result"]["receipts"][0]["write_kind"],
            "emit_run_event"
        );
    }

    ctx.checkpoint_with_optimizer_state(
        OptimizerStateWrite::json(
            Fingerprint::from_bytes([93; 32]),
            Fingerprint::from_bytes([94; 32]),
            &json!({"boundary": "run-bound-runtime-stdio"}),
        )
        .unwrap(),
    )
    .unwrap();
    drop(ctx);

    let restored = persistence
        .latest_checkpoint::<RunBoundProblem>()
        .unwrap()
        .expect("latest checkpoint restores after stdio-routed callbacks");
    let mut restored_graph = restored.graph;
    let mut restored_budget = restored.budget;
    let restored_ctx =
        RunContext::<RunBoundProblem>::new(&mut restored_graph, &mut restored_budget);
    let graph = restored_ctx.graph();
    assert_eq!(graph.candidate_count(), 2);
    assert_eq!(graph.evaluation_request_count(), 1);
    assert_eq!(graph.assessment_count(), 1);
    assert!(graph.events().any(|event| matches!(
        event,
        RunEvent::ExternalEventEmitted { event_kind, .. }
            if event_kind == "run_bound.checked"
    )));
}

#[test]
fn engine_lifecycle_mounts_run_bound_service_and_checkpoint_readback_sees_graph_truth() {
    let temp = TempDir::new().unwrap();
    let store = FileStore::open(temp.path()).unwrap();
    let persistence = StoreRunPersistence::new(store);
    let evidence_store = InlineEvidenceStore::<RunBoundEvidence>::new("run-bound");
    let case_set = CaseSet::new(vec![RunBoundCase]);
    let mut engine = leaven_engine::Engine::<RunBoundProblem>::builder()
        .budget(Budget::unlimited())
        .persistence(persistence.clone())
        .build();
    let seed = engine.insert_seed(RunBoundArtifact(1), 0).unwrap();

    let mut optimizer = SeamMountedOptimizer {
        seed,
        mounted: false,
    };
    futures::executor::block_on(engine.run(&mut optimizer, &case_set, &evidence_store)).unwrap();

    assert!(
        optimizer.mounted,
        "optimizer step should mount seam service"
    );
    let restored = persistence
        .latest_checkpoint::<RunBoundProblem>()
        .unwrap()
        .expect("engine lifecycle should advance latest checkpoint");
    let mut restored_graph = restored.graph;
    let mut restored_budget = restored.budget;
    let restored_ctx =
        RunContext::<RunBoundProblem>::new(&mut restored_graph, &mut restored_budget);
    let graph = restored_ctx.graph();
    assert_eq!(graph.candidate_count(), 2);
    assert_eq!(graph.evaluation_request_count(), 1);
    assert_eq!(graph.assessment_count(), 1);
    assert!(graph.events().any(|event| matches!(
        event,
        RunEvent::ExternalEventEmitted { event_kind, .. }
            if event_kind == "run_bound.checked"
    )));
}

fn jsonrpc_request(id: &str, method: &str, params: Value) -> Value {
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

fn prefixed_jcs_hash(prefix: &str, value: &Value) -> String {
    let digest = jcs_canonicalize::sha256_jcs_hex(value).unwrap();
    format!("{prefix}{digest}")
}

fn proposal_apply_request(batch_ref: &str) -> Value {
    json!({
        "schema_version": "leaven.plan.v1",
        "plan_id": "plan_run_bound_apply",
        "consistency": {
            "kind": "latest_at_start"
        },
        "mode": {
            "kind": "execute"
        },
        "ops": [{
            "kind": "write",
            "name": "apply",
            "idempotency_key": "run-bound-apply-0001",
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

fn evaluation_request_request() -> Value {
    json!({
        "schema_version": "leaven.plan.v1",
        "plan_id": "plan_run_bound_evaluation",
        "consistency": {
            "kind": "latest_at_start"
        },
        "mode": {
            "kind": "execute"
        },
        "ops": [{
            "kind": "write",
            "name": "evaluation_request",
            "idempotency_key": "run-bound-evaluation-0001",
            "write": {
                "kind": "request_evaluation",
                "request": {
                    "evaluator": "eval_run_bound",
                    "shape": "independent",
                    "candidates": ["cand_placeholder"],
                    "set": {
                        "kind": "named",
                        "name": "validation"
                    },
                    "granularity": "per_case",
                    "purpose": "validation"
                }
            }
        }],
        "return": ["evaluation_request"],
        "commit": {
            "kind": "graph_writes_atomic",
            "on_stale": "reject"
        }
    })
}

fn assessment_submit_request(evaluation_request_id: &str) -> Value {
    json!({
        "schema_version": "leaven.plan.v1",
        "plan_id": "plan_run_bound_assessment",
        "consistency": {
            "kind": "latest_at_start"
        },
        "mode": {
            "kind": "execute"
        },
        "ops": [{
            "kind": "write",
            "name": "assessment_batch",
            "idempotency_key": "run-bound-assessment-0001",
            "write": {
                "kind": "submit_assessments",
                "evaluation_request_id": evaluation_request_id,
                "assessments": [{
                    "kind": "independent",
                    "candidate": "cand_a",
                    "target": {
                        "case": "case_1"
                    },
                    "score": {
                        "value": 1.0,
                        "output": {
                            "kind": "structured",
                            "summary": "candidate answered correctly",
                            "value": {
                                "candidate": "cand_a",
                                "output": "candidate answered correctly"
                            },
                            "visibility": "public",
                            "data_classes": ["candidate.output"]
                        }
                    },
                    "evidence": {
                        "schema_version": "leaven.evidence_envelope.v1",
                        "target_derived": false,
                        "public": {
                            "summary": "candidate answered correctly",
                            "data_classes": ["public"]
                        },
                        "redaction_policy": {
                            "optimizer": "score_only",
                            "reflector": "score_only",
                            "operator": "score_only"
                        },
                        "producer": {
                            "stage_call_id": "sc_run_bound_assessment"
                        },
                        "source_receipts": {
                            "read": ["qrec_run_bound_assessment_source"],
                            "effect": []
                        }
                    },
                    "replayability": "pure_read"
                }]
            }
        }],
        "return": ["assessment_batch"],
        "commit": {
            "kind": "graph_writes_atomic",
            "on_stale": "reject"
        }
    })
}

fn event_emit_request() -> Value {
    json!({
        "schema_version": "leaven.plan.v1",
        "plan_id": "plan_run_bound_event",
        "consistency": {
            "kind": "latest_at_start"
        },
        "mode": {
            "kind": "execute"
        },
        "ops": [{
            "kind": "write",
            "name": "run_bound_event",
            "idempotency_key": "run-bound-event-0001",
            "write": {
                "kind": "emit_run_event",
                "event_kind": "run_bound.checked",
                "payload_schema": "fp_schema_sha256_run_bound_test",
                "payload": {"ok": true},
                "visibility": "public"
            }
        }],
        "return": ["run_bound_event"],
        "commit": {
            "kind": "graph_writes_atomic",
            "on_stale": "reject"
        }
    })
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct RunBoundArtifact(i32);

impl Artifact for RunBoundArtifact {
    type Change = i32;
    type ApplyError = RunBoundApplyError;

    fn identity(&self) -> ArtifactIdentity {
        ArtifactIdentity::Content(ContentId::hash_bytes(self.0.to_le_bytes()))
    }

    fn apply_change(&self, change: &Self::Change) -> Result<Self, Self::ApplyError> {
        Ok(Self(self.0 + change))
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct RunBoundEvidence;

impl Evidence for RunBoundEvidence {}

#[derive(Clone, Debug)]
struct RunBoundCase;

struct RunBoundProblem;

impl OptimizationProblem for RunBoundProblem {
    type Artifact = RunBoundArtifact;
    type Case = RunBoundCase;
    type Evidence = RunBoundEvidence;
    type ProposalAnnotations = ();
}

#[derive(Debug)]
struct RunBoundApplyError;

impl fmt::Display for RunBoundApplyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("run-bound apply failed")
    }
}

impl Error for RunBoundApplyError {}

struct SeamMountedOptimizer {
    seed: CandidateId,
    mounted: bool,
}

impl Optimizer<RunBoundProblem> for SeamMountedOptimizer {
    async fn step(
        &mut self,
        ctx: &mut RunContext<'_, RunBoundProblem>,
    ) -> Result<StepStatus, OptimizerError> {
        let package = leaven_public_seam::PublicSeamPackage::active_from_repo(workspace_root())
            .map_err(|source| OptimizerError::with_source("load public seam package", source))?;
        let seed = self.seed;
        let batch = ctx
            .record_proposal_batch(
                StageId::custom("engine-mounted-run-bound-proposer"),
                ProposalBatch {
                    proposals: vec![Proposal::mutate(seed, 17).build()],
                    semantics: ProposalBatchSemantics::Alternatives,
                    metadata: MetadataBag::new(),
                },
                Cost::zero(),
            )
            .map_err(|source| OptimizerError::with_source("record proposal batch", source))?;
        let batch_ref = format!("pb_{}", batch.batch_id.as_uuid());
        {
            let service = RunBoundGraphEffectService::new(
                ctx,
                [batch],
                "fp_cap_sha256_engine_mounted",
                "fp_policy_sha256_engine_mounted",
                "rev_engine_mounted_base",
                "rev_engine_mounted_final",
            )
            .with_evaluation_requester({
                move |_params| {
                    Ok(RunBoundEvaluationRequest {
                        evaluator: EvaluatorId::from("eval_engine_mounted"),
                        evaluator_fingerprint: Fingerprint::from_bytes([38; 32]),
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
                            evidence: RunBoundEvidence,
                            cost: Cost::zero(),
                            metadata: MetadataBag::new(),
                        }],
                        Cost::zero(),
                    ))
                }
            });
            let runtime = SeamRuntime::from_package(package, service)
                .map_err(|source| OptimizerError::with_source("build seam runtime", source))?;
            let first_lines = serve_jsonrpc_lines(
                &runtime,
                [
                    jsonrpc_request(
                        "engine-mounted-apply",
                        "leaven/proposal.apply",
                        proposal_apply_request(&batch_ref),
                    ),
                    jsonrpc_request(
                        "engine-mounted-evaluation",
                        "leaven/evaluation.request",
                        evaluation_request_request(),
                    ),
                ],
            )?;
            let evaluation_ref = first_lines[1]["result"]["primary"]["evaluation_request_id"]
                .as_str()
                .ok_or_else(|| {
                    OptimizerError::Message(
                        "engine-mounted evaluation response missing request id".to_owned(),
                    )
                })?
                .to_owned();
            serve_jsonrpc_lines(
                &runtime,
                [
                    jsonrpc_request(
                        "engine-mounted-assessment",
                        "leaven/assessment.submit",
                        assessment_submit_request(&evaluation_ref),
                    ),
                    jsonrpc_request(
                        "engine-mounted-event",
                        "leaven/event.emit",
                        event_emit_request(),
                    ),
                ],
            )?;
        }
        self.mounted = true;
        Ok(StepStatus::Done)
    }

    fn best_candidate(
        &self,
        _graph: leaven_engine::RunGraphView<'_, RunBoundProblem>,
    ) -> Option<CandidateId> {
        None
    }
}

fn serve_jsonrpc_lines<const N: usize>(
    runtime: &SeamRuntime<impl leaven_seam_runtime::SeamService>,
    requests: [Value; N],
) -> Result<Vec<Value>, OptimizerError> {
    let input = requests
        .into_iter()
        .map(|request| request.to_string())
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    let mut output = Vec::new();
    serve_reader_writer(runtime, Cursor::new(input), &mut output)
        .map_err(|source| OptimizerError::with_source("serve run-bound stdio lines", source))?;
    let lines = response_lines(output);
    if lines.iter().any(|line| line.get("error").is_some()) {
        return Err(OptimizerError::Message(format!(
            "run-bound stdio returned errors: {lines:?}"
        )));
    }
    Ok(lines)
}

fn workspace_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("crate has workspace root ancestor")
        .to_path_buf()
}
