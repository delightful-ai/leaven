use std::error::Error;
use std::fmt;

use leaven_core::{
    Artifact, ArtifactIdentity, Assessment, AssessmentGranularity, AssessmentTarget,
    EvaluationPurpose, EvaluationRequest, EvaluationSet, Evidence, OptimizationProblem, Proposal,
    ProposalBatch, ProposalBatchSemantics,
};
use leaven_engine::{
    BudgetLedger, CaseSet, OptimizerStateWrite, RunContext, RunEvent, StoreRunPersistence,
};
use leaven_kernel::{
    Budget, ContentId, Cost, EvaluatorId, Fingerprint, MetadataBag, Metered, StageId,
};
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

        let apply = service
            .handle_method("leaven/proposal.apply", &proposal_apply_request(&batch_ref))
            .unwrap();
        assert_eq!(apply["primary"]["kind"], "apply_receipt");
        assert_eq!(apply["receipts"][0]["write_kind"], "apply_proposal_batch");
        package
            .validate_acp_extension_result_document(&apply)
            .expect("apply result validates through public seam owner");

        let evaluation = service
            .handle_method("leaven/evaluation.request", &evaluation_request_request())
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
                "leaven/assessment.submit",
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
            .handle_method("leaven/event.emit", &event_emit_request())
            .unwrap();
        assert_eq!(event["primary"]["kind"], "emit_run_event");
        assert_eq!(event["receipts"][0]["write_kind"], "emit_run_event");
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
            "leaven/proposal.apply",
            &proposal_apply_request("pb_configured_run_context"),
        )
        .unwrap_err();

    assert!(matches!(
        error,
        RunBoundGraphEffectError::InvalidProposalBatchRef
    ));
}

fn proposal_apply_request(batch_ref: &str) -> Value {
    json!({
        "schema_version": "leaven.plan.v1",
        "plan_id": "plan_run_bound_apply",
        "mode": "live",
        "base_revision": "rev_run_bound_base",
        "ops": [{
            "name": "apply",
            "write": {
                "kind": "apply_proposal_batch",
                "idempotency_key": "run-bound-apply-0001",
                "proposal_batch": batch_ref,
                "base_revision": "rev_run_bound_base",
                "preconditions": []
            }
        }],
        "return": ["apply"]
    })
}

fn evaluation_request_request() -> Value {
    json!({
        "schema_version": "leaven.plan.v1",
        "plan_id": "plan_run_bound_evaluation",
        "mode": "live",
        "base_revision": "rev_run_bound_base",
        "ops": [{
            "name": "evaluation_request",
            "write": {
                "kind": "request_evaluation",
                "idempotency_key": "run-bound-evaluation-0001",
                "request": {
                    "evaluator": "eval_run_bound",
                    "shape": "independent",
                    "candidates": ["cand_placeholder"],
                    "case_set": {"kind": "all"},
                    "granularity": "per_case",
                    "purpose": "validation"
                },
                "base_revision": "rev_run_bound_base",
                "capability": "cap_run_bound"
            }
        }],
        "return": ["evaluation_request"]
    })
}

fn assessment_submit_request(evaluation_request_id: &str) -> Value {
    json!({
        "schema_version": "leaven.plan.v1",
        "plan_id": "plan_run_bound_assessment",
        "mode": "live",
        "base_revision": "rev_run_bound_base",
        "ops": [{
            "name": "assessment_batch",
            "write": {
                "kind": "submit_assessments",
                "idempotency_key": "run-bound-assessment-0001",
                "evaluation_request_id": evaluation_request_id,
                "assessments": [],
                "base_revision": "rev_run_bound_base"
            }
        }],
        "return": ["assessment_batch"]
    })
}

fn event_emit_request() -> Value {
    json!({
        "schema_version": "leaven.plan.v1",
        "plan_id": "plan_run_bound_event",
        "mode": "live",
        "base_revision": "rev_run_bound_base",
        "ops": [{
            "name": "run_bound_event",
            "write": {
                "kind": "emit_run_event",
                "idempotency_key": "run-bound-event-0001",
                "event_kind": "run_bound.checked",
                "payload_schema": "leaven.run_bound_test.v1",
                "payload": {"ok": true},
                "visibility": "public",
                "base_revision": "rev_run_bound_base"
            }
        }],
        "return": ["run_bound_event"]
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

fn workspace_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("crate has workspace root ancestor")
        .to_path_buf()
}
