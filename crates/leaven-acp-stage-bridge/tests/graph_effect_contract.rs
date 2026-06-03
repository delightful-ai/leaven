//! Focused contract for worker-initiated graph-effect callbacks.

use std::convert::Infallible;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use futures::executor::block_on;
use leaven_acp::{AcpProcessCommand, AcpStdioProcessSession};
use leaven_acp_stage_bridge::{ExternalEvaluationRequest, RunContextGraphEffectHost};
use leaven_core::{
    Artifact, ArtifactIdentity, Assessment, AssessmentGranularity, AssessmentTarget,
    EvaluationPurpose, EvaluationRequest, EvaluationSet, Evidence, OptimizationProblem,
    ResolvedEvaluationRequest,
};
use leaven_engine::{
    BudgetLedger, CaseSet, EvaluationContext, EvaluationError, Evaluator, RunContext, RunEvent,
    RunGraph,
};
use leaven_kernel::{
    Budget, CandidateId, Cost, EvaluatorId, Fingerprint, MetadataBag, Metered, RunId,
};
use leaven_public_seam::{AcpProfileDocument, PublicSeamPackage};
use leaven_store_inline::InlineEvidenceStore;
use serde_json::{Value, json};
use tempfile::TempDir;
use uuid::Uuid;

#[test]
fn dispatch_stage_run_services_worker_initiated_assessment_submit_through_runcontext() {
    block_on(async {
        let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
        let profile = acp_profile(&package);
        let mut graph = RunGraph::<BridgeProblem>::new(RunId::new());
        let mut budget = BudgetLedger::new(Budget::unlimited());
        let store = InlineEvidenceStore::<BridgeEvidence>::new("bridge");
        let case_set = CaseSet::new(vec![()]);
        let candidate = {
            let mut context = RunContext::<BridgeProblem>::new(&mut graph, &mut budget);
            context.insert_seed(BridgeArtifact(5), 0).unwrap()
        };
        let request_id = {
            let mut context =
                RunContext::<BridgeProblem>::new(&mut graph, &mut budget).with_case_set(&case_set);
            context
                .evaluate_with(&BridgeFailingEvaluator, independent_request(candidate))
                .await
                .unwrap_err();
            context
                .graph()
                .events()
                .find_map(|event| match event {
                    RunEvent::EvaluationRequested { request_id, .. } => Some(*request_id),
                    _ => None,
                })
                .expect("failing evaluator records a request for external assessment completion")
        };
        let public_request = format!("evalreq_{}", request_id.as_uuid());
        let public_candidate = format!("cand_{}", candidate.as_uuid());
        let script = python_worker(&format!(
            r#"
import json
import os
import sys

request = json.loads(sys.stdin.readline())
payload = request["params"]["payload"]

print(json.dumps({{
    "jsonrpc": "2.0",
    "id": "%s::assessment" % payload["stage_call_id"],
    "method": "leaven/assessment.submit",
    "params": {{
        "schema_version": "leaven.plan.v1",
        "plan_id": "plan_runner_assessment_submit",
        "consistency": {{"kind": "latest_at_start"}},
        "mode": {{"kind": "execute"}},
        "ops": [{{
            "kind": "write",
            "name": "assessment_batch",
            "idempotency_key": "assessment-submit-stage-bridge-0001",
            "write": {{
                "kind": "submit_assessments",
                "evaluation_request_id": "{public_request}",
                "assessments": [{{
                    "kind": "independent",
                    "candidate": "{public_candidate}",
                    "target": {{"case": "case_1"}},
                    "score": {{
                        "value": 0.875,
                        "output": {{
                            "kind": "structured",
                            "summary": "worker scored candidate",
                            "value": {{"candidate": "{public_candidate}", "output": "worker score"}},
                            "visibility": "public",
                            "data_classes": ["candidate.output"]
                        }}
                    }},
                    "evidence": {{
                        "schema_version": "leaven.evidence_envelope.v1",
                        "target_derived": False,
                        "public": {{
                            "summary": "worker scored candidate",
                            "data_classes": ["public"]
                        }},
                        "redaction_policy": {{
                            "optimizer": "score_only",
                            "reflector": "score_only",
                            "operator": "score_only"
                        }},
                        "producer": {{"stage_call_id": payload["stage_call_id"]}},
                        "source_receipts": {{
                            "read": ["qrec_assessment_source"],
                            "effect": []
                        }}
                    }},
                    "replayability": "pure_read"
                }}]
            }},
        }}],
        "return": ["assessment_batch"],
        "commit": {{"kind": "graph_writes_atomic", "on_stale": "reject"}},
    }},
}}), flush=True)

reply = json.loads(sys.stdin.readline())

print(json.dumps({{
    "jsonrpc": "2.0",
    "id": request["id"],
    "result": {{
        "schema_version": "leaven.stage_run.v1",
        "message": "stage_run_result",
        "stage": "runner",
        "stage_call_id": payload["stage_call_id"],
        "output": {{
            "kind": "text",
            "summary": "assessment submitted",
            "value": json.dumps(reply),
            "visibility": "optimizer_visible",
            "data_classes": ["candidate.output"],
        }},
    }},
}}), flush=True)
"#
        ));
        let mut session = spawn(&script, package, profile);
        let mut context =
            RunContext::<BridgeProblem>::new(&mut graph, &mut budget).with_evidence_store(&store);
        let host = RunContextGraphEffectHost::new(
            &mut context,
            [],
            "fp_cap_sha256_stage_bridge",
            "fp_policy_sha256_stage_bridge",
            "rev_stage_bridge_base",
            "rev_stage_bridge_final",
        )
        .with_assessment_submitter(move |params| lower_bridge_assessment(params, candidate));

        let response = session
            .dispatch_stage_run(&runner_request("assessment submit"), &host)
            .unwrap();

        let callback_reply: Value = serde_json::from_str(
            response.result().output().as_value()["value"]
                .as_str()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(
            callback_reply["result"]["method"], "leaven/assessment.submit",
            "{callback_reply}"
        );
        assert_eq!(
            callback_reply["result"]["primary"]["kind"], "assessment_batch_receipt",
            "{callback_reply}"
        );
        assert_eq!(
            callback_reply["result"]["receipts"][0]["write_kind"], "submit_assessments",
            "{callback_reply}"
        );
        drop(host);
        assert_eq!(context.graph().assessment_count(), 1);
        let assessment = context
            .graph()
            .all_assessments()
            .next()
            .expect("worker callback records an assessment");
        assert_eq!(assessment.request_id(), request_id);
        assert_eq!(assessment.independent_candidate(), Some(candidate));
        assert_eq!(
            context.assessment_evidence(assessment.id()).unwrap().score,
            0.875
        );
        forget(script);
    });
}

#[test]
fn dispatch_stage_run_services_worker_initiated_evaluation_request_through_runcontext() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let profile = acp_profile(&package);
    let mut graph = RunGraph::<BridgeProblem>::new(RunId::new());
    let mut budget = BudgetLedger::new(Budget::unlimited());
    let case_set = CaseSet::new(vec![()]);
    let candidate = {
        let mut context = RunContext::<BridgeProblem>::new(&mut graph, &mut budget);
        context.insert_seed(BridgeArtifact(9), 0).unwrap()
    };
    let public_candidate = format!("cand_{}", candidate.as_uuid());
    let script = python_worker(&format!(
        r#"
import json
import sys

request = json.loads(sys.stdin.readline())
payload = request["params"]["payload"]

print(json.dumps({{
    "jsonrpc": "2.0",
    "id": "%s::evaluation" % payload["stage_call_id"],
    "method": "leaven/evaluation.request",
    "params": {{
        "schema_version": "leaven.plan.v1",
        "plan_id": "plan_runner_evaluation_request",
        "consistency": {{"kind": "latest_at_start"}},
        "mode": {{"kind": "execute"}},
        "ops": [{{
            "kind": "write",
            "name": "evaluation_request",
            "idempotency_key": "evaluation-request-stage-bridge-0001",
            "write": {{
                "kind": "request_evaluation",
                "request": {{
                    "shape": "independent",
                    "candidates": ["{public_candidate}"],
                    "set": {{"kind": "named", "name": "validation"}},
                    "granularity": "per_case",
                    "purpose": "validation",
                    "evaluator": "bridge_eval_v1"
                }}
            }},
        }}],
        "return": ["evaluation_request"],
        "commit": {{"kind": "graph_writes_atomic", "on_stale": "reject"}},
    }},
}}), flush=True)

reply = json.loads(sys.stdin.readline())

print(json.dumps({{
    "jsonrpc": "2.0",
    "id": request["id"],
    "result": {{
        "schema_version": "leaven.stage_run.v1",
        "message": "stage_run_result",
        "stage": "runner",
        "stage_call_id": payload["stage_call_id"],
        "output": {{
            "kind": "text",
            "summary": "evaluation requested",
            "value": json.dumps(reply),
            "visibility": "optimizer_visible",
            "data_classes": ["candidate.output"],
        }},
    }},
}}), flush=True)
"#
    ));
    let mut session = spawn(&script, package, profile);
    let mut context =
        RunContext::<BridgeProblem>::new(&mut graph, &mut budget).with_case_set(&case_set);
    let host = RunContextGraphEffectHost::new(
        &mut context,
        [],
        "fp_cap_sha256_stage_bridge",
        "fp_policy_sha256_stage_bridge",
        "rev_stage_bridge_base",
        "rev_stage_bridge_final",
    )
    .with_evaluation_requester(move |params| lower_bridge_evaluation_request(params, candidate));

    let response = session
        .dispatch_stage_run(&runner_request("evaluation request"), &host)
        .unwrap();

    let callback_reply: Value = serde_json::from_str(
        response.result().output().as_value()["value"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        callback_reply["result"]["method"], "leaven/evaluation.request",
        "{callback_reply}"
    );
    assert_eq!(
        callback_reply["result"]["primary"]["kind"], "evaluation_request_receipt",
        "{callback_reply}"
    );
    assert_eq!(
        callback_reply["result"]["receipts"][0]["write_kind"], "request_evaluation",
        "{callback_reply}"
    );
    drop(host);
    assert_eq!(context.graph().evaluation_request_count(), 1);
    let request_id = context
        .graph()
        .events()
        .find_map(|event| match event {
            RunEvent::EvaluationRequested { request_id, .. } => Some(*request_id),
            _ => None,
        })
        .expect("worker callback records an evaluation request event");
    let request = context
        .graph()
        .evaluation_request(request_id)
        .expect("worker callback records evaluation request in graph");
    assert_eq!(request.evaluator(), &EvaluatorId::from("bridge_eval_v1"));
    assert_eq!(request.resolved_set().case_ids.len(), 1);
    forget(script);
}

fn lower_bridge_assessment(
    params: &Value,
    expected_candidate: CandidateId,
) -> Result<Metered<Vec<Assessment<BridgeProblem>>>, String> {
    let write = params["ops"]
        .as_array()
        .and_then(|ops| ops.iter().find_map(|op| op.get("write")))
        .ok_or_else(|| "missing write".to_owned())?;
    let assessment = write["assessments"]
        .as_array()
        .and_then(|assessments| assessments.first())
        .ok_or_else(|| "missing assessment".to_owned())?;
    let candidate = parse_candidate_ref(
        assessment["candidate"]
            .as_str()
            .ok_or_else(|| "missing candidate".to_owned())?,
    )?;
    if candidate != expected_candidate {
        return Err("assessment candidate did not match request candidate".to_owned());
    }
    let score = assessment["score"]["value"]
        .as_f64()
        .ok_or_else(|| "missing score.value".to_owned())?;
    Ok(Metered::new(
        vec![Assessment::Independent {
            candidate,
            target: AssessmentTarget::Unscoped,
            evidence: BridgeEvidence { score },
            cost: Cost::zero(),
            metadata: MetadataBag::new(),
        }],
        Cost::metric_calls(1),
    ))
}

fn lower_bridge_evaluation_request(
    params: &Value,
    expected_candidate: CandidateId,
) -> Result<ExternalEvaluationRequest, String> {
    let request = params["ops"]
        .as_array()
        .and_then(|ops| ops.iter().find_map(|op| op.get("write")))
        .and_then(|write| write.get("request"))
        .ok_or_else(|| "missing request_evaluation.request".to_owned())?;
    let candidate = request["candidates"]
        .as_array()
        .and_then(|candidates| candidates.first())
        .and_then(Value::as_str)
        .ok_or_else(|| "missing request candidate".to_owned())
        .and_then(parse_candidate_ref)?;
    if candidate != expected_candidate {
        return Err("evaluation request candidate did not match host candidate".to_owned());
    }
    Ok(ExternalEvaluationRequest {
        evaluator: EvaluatorId::from(
            request["evaluator"]
                .as_str()
                .ok_or_else(|| "missing evaluator".to_owned())?
                .to_owned(),
        ),
        evaluator_fingerprint: Fingerprint::from_bytes([37; 32]),
        request: EvaluationRequest::Independent {
            candidates: vec![candidate],
            set: EvaluationSet::All,
            granularity: AssessmentGranularity::PerCase,
            purpose: EvaluationPurpose::Validation,
        },
    })
}

fn parse_candidate_ref(value: &str) -> Result<CandidateId, String> {
    value
        .strip_prefix("cand_")
        .and_then(|raw| Uuid::parse_str(raw).ok())
        .map(CandidateId::from_uuid)
        .ok_or_else(|| "candidate must be a cand_<uuid> ref".to_owned())
}

fn independent_request(candidate: CandidateId) -> EvaluationRequest {
    EvaluationRequest::Independent {
        candidates: vec![candidate],
        set: EvaluationSet::All,
        granularity: AssessmentGranularity::Aggregate,
        purpose: EvaluationPurpose::Validation,
    }
}

fn runner_request(prompt: &str) -> Value {
    json!({
        "schema_version": "leaven.stage_run.v1",
        "message": "stage_run_request",
        "stage": "runner",
        "payload": {
            "schema_version": "leaven.stage_payloads.v1",
            "role": "runner",
            "run": "run_graph_effect_contract",
            "stage_call_id": "sc_graph_effect_contract",
            "candidate": "cand_graph_effect_contract",
            "case": "case_graph_effect_contract",
            "case_input": {"question": "score", "prompt": prompt},
            "target_forbidden": true
        }
    })
}

fn python_worker(body: &str) -> TempDir {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("worker.py");
    fs::write(&path, body).unwrap();
    assert!(
        Command::new("python3")
            .arg("-m")
            .arg("py_compile")
            .arg(&path)
            .status()
            .unwrap()
            .success()
    );
    temp
}

fn spawn(
    script: &TempDir,
    package: PublicSeamPackage,
    profile: AcpProfileDocument,
) -> AcpStdioProcessSession {
    let path = script.path().join("worker.py");
    AcpStdioProcessSession::spawn(
        package,
        profile,
        AcpProcessCommand::new("python3").arg(path.to_str().unwrap()),
        "secret-token",
        "stdio://worker/session",
        "fp_cap_sha256_stage_bridge",
    )
    .unwrap()
}

fn forget(script: TempDir) {
    std::mem::forget(script);
}

fn acp_profile(package: &PublicSeamPackage) -> AcpProfileDocument {
    let value = json!({
        "schema_version": "leaven.acp_profile.v1",
        "base_protocol": "agent-client-protocol",
        "pinned_acp_version": "0.4.0",
        "transports": ["stdio_jsonrpc", "unix_socket_jsonrpc"],
        "auth": {
            "token_env": "LEAVEN_CAPABILITY_TOKEN",
            "endpoint_env": "LEAVEN_ENDPOINT",
            "fingerprint_env": "LEAVEN_CAPABILITY_FINGERPRINT",
            "http_header": "Authorization: Bearer <token>",
            "authenticate_maps_to": "leaven.capability.v1"
        },
        "permission_model": {
            "source": "ACP session/request_permission",
            "answer": "programmatic capability grant check",
            "denial": "PlanError + Redaction"
        },
        "extension_methods": locked_profile_methods(),
        "flow_control": {
            "bounded_channel_required": true,
            "default_max_inflight_updates": 32,
            "backpressure": "pause_worker",
            "heartbeat_ms": 1000
        }
    });
    package.validate_acp_profile_document(&value).unwrap()
}

fn locked_profile_methods() -> Vec<Value> {
    let mut methods = vec![json!({
        "method": "leaven/stage.run",
        "params_schema": "leaven.stage_run.v1.schema.json",
        "result_schema": "leaven.stage_run.v1.schema.json",
        "required_action": "stage.run",
        "produces_receipt": true
    })];
    for (method, action) in [
        ("leaven/graph.query", "graph.query"),
        ("leaven/case.load", "case.read"),
        ("leaven/case.input", "case.read"),
        ("leaven/case.target", "case.read"),
        ("leaven/case.metadata", "case.read"),
        ("leaven/workspace.materialize", "workspace.materialize"),
        ("leaven/workspace.snapshot", "workspace.read"),
        ("leaven/workspace.list", "workspace.read"),
        ("leaven/workspace.read_file", "workspace.read"),
        ("leaven/workspace.stat", "workspace.read"),
        ("leaven/workspace.digest", "workspace.read"),
        ("leaven/workspace.git_log", "workspace.read"),
        ("leaven/workspace.git_diff", "workspace.read"),
        ("leaven/workspace.git_status", "workspace.read"),
        ("leaven/workspace.capture_artifacts", "workspace.read"),
        ("leaven/workspace.release", "workspace.release"),
        ("leaven/lm.complete", "lm.complete"),
        ("leaven/agent.run", "agent.run"),
        ("leaven/sandbox.exec", "sandbox.exec"),
        ("leaven/proposal.submit_batch", "proposal.submit_batch"),
        ("leaven/proposal.apply", "proposal.apply_batch"),
        ("leaven/assessment.submit", "assessment.submit"),
        ("leaven/evaluation.request", "evaluation.request"),
        ("leaven/event.emit", "event.emit"),
    ] {
        methods.push(json!({
            "method": method,
            "params_schema": "leaven.plan.v1.schema.json",
            "result_schema": "leaven.plan_result.v1.schema.json",
            "required_action": action,
            "produces_receipt": true
        }));
    }
    methods
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .unwrap()
        .to_path_buf()
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct BridgeArtifact(i32);

impl Artifact for BridgeArtifact {
    type Change = i32;
    type ApplyError = Infallible;

    fn identity(&self) -> ArtifactIdentity {
        ArtifactIdentity::External(format!("bridge-artifact-{}", self.0))
    }

    fn apply_change(&self, change: &Self::Change) -> Result<Self, Self::ApplyError> {
        Ok(Self(self.0 + change))
    }
}

struct BridgeProblem;

impl OptimizationProblem for BridgeProblem {
    type Artifact = BridgeArtifact;
    type Case = ();
    type Evidence = BridgeEvidence;
    type ProposalAnnotations = ();
}

#[derive(Clone)]
struct BridgeEvidence {
    score: f64,
}

impl Evidence for BridgeEvidence {}

struct BridgeFailingEvaluator;

impl Evaluator<BridgeProblem> for BridgeFailingEvaluator {
    fn id(&self) -> EvaluatorId {
        EvaluatorId::from("bridge-fail")
    }

    fn fingerprint(&self) -> Fingerprint {
        Fingerprint::from_bytes([23; 32])
    }

    async fn evaluate(
        &self,
        _request: ResolvedEvaluationRequest,
        _ctx: EvaluationContext<'_, BridgeProblem>,
    ) -> Result<Metered<Vec<Assessment<BridgeProblem>>>, EvaluationError> {
        Err(EvaluationError::Message(
            "external worker will submit assessments".to_owned(),
        ))
    }
}
