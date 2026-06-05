//! Focused contract for the stage-dispatch + bidirectional-callback leg.
//!
//! These tests isolate the transport-level proof from the tiny GEPA loop:
//! `dispatch_stage_run` carries a runner stage to a worker that initiates
//! `leaven/lm.complete` back, the mock host answers, and the worker's text stage
//! result is parsed. The negatives prove the dispatch refuses a non-text stage
//! output and refuses target material smuggled into the runner request.

use std::convert::Infallible;
use std::fs;
use std::path::{Path, PathBuf};

use leaven_acp::{AcpProcessCommand, AcpStdioProcessSession, RejectAllEffectHost};
use leaven_acp_stage_bridge::{
    MockArithmeticLm, OptimizeConfig, PromptArtifact, RunContextGraphEffectHost, RunnerDispatch,
    StageRunEffectHost, optimize_prompt,
};
use leaven_core::{
    Artifact, ArtifactIdentity, Evidence, OptimizationProblem, Proposal, ProposalBatch,
    ProposalBatchSemantics,
};
use leaven_engine::{BudgetLedger, RunContext, RunEvent, RunGraph};
use leaven_kernel::{Budget, Cost, MetadataBag, RunId, StageId};
use leaven_public_seam::{AcpProfileDocument, PublicSeamPackage};
use serde_json::{Value, json};
use tempfile::TempDir;

#[test]
fn dispatch_stage_run_services_worker_initiated_lm_complete_and_parses_text_output() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let profile = acp_profile(&package);
    let script = python_worker(
        r#"
import json
import os
import sys

request = json.loads(sys.stdin.readline())
assert request["method"] == "leaven/stage.run", request
payload = request["params"]["payload"]
assert payload["role"] == "runner", payload
assert payload["target_forbidden"] is True, payload
prompt = payload["case_input"]["prompt"]

# Worker is the ACP agent: call leaven/lm.complete back into the host.
print(json.dumps({
    "jsonrpc": "2.0",
    "id": "%s::lm" % payload["stage_call_id"],
    "method": "leaven/lm.complete",
    "params": {
        "schema_version": "leaven.plan.v1",
        "plan_id": "plan_runner_lm_complete",
        "consistency": {"kind": "latest_at_start"},
        "mode": {"kind": "dry_run"},
        "ops": [{
            "kind": "let",
            "name": "prompt",
            "expr": {"kind": "literal", "value": prompt, "data_classes": ["public"]},
        }],
        "return": ["prompt"],
        "commit": {"kind": "no_graph_writes"},
    },
}), flush=True)

reply = json.loads(sys.stdin.readline())
assert reply["result"]["method"] == "leaven/lm.complete", reply
assert reply["result"]["capability_fingerprint"] == os.environ["LEAVEN_CAPABILITY_FINGERPRINT"], reply
text = "".join(p["text"] for p in reply["result"]["primary"]["message"]["content"])

print(json.dumps({
    "jsonrpc": "2.0",
    "id": request["id"],
    "result": {
        "schema_version": "leaven.stage_run.v1",
        "message": "stage_run_result",
        "stage": "runner",
        "stage_call_id": payload["stage_call_id"],
        "output": {
            "kind": "text",
            "summary": "runner output",
            "value": text.strip(),
            "visibility": "optimizer_visible",
            "data_classes": ["candidate.output"],
        },
    },
}), flush=True)
"#,
    );
    let mut session = spawn(&script, package, profile);
    let lm = MockArithmeticLm;
    let host = StageRunEffectHost::new(&lm);
    let response = session
        .dispatch_stage_run(
            &runner_request("5 + 7 = ?\nExpression: 5 + 7\nAnswer:"),
            &host,
        )
        .unwrap();
    assert_eq!(response.result().output().kind(), "text");
    assert_eq!(
        response.result().output().as_value()["value"],
        json!("12"),
        "the worker returned the mock LM's evaluation over the seam"
    );
    forget(script);
}

#[test]
fn dispatch_stage_run_services_worker_initiated_proposal_apply_through_runcontext() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let profile = acp_profile(&package);
    let mut graph = RunGraph::<BridgeProblem>::new(RunId::new());
    let mut budget = BudgetLedger::new(Budget::unlimited());
    let mut context = RunContext::<BridgeProblem>::new(&mut graph, &mut budget);
    let seed = context.insert_seed(BridgeArtifact(1), 0).unwrap();
    let batch = context
        .record_proposal_batch(
            StageId::custom("acp-stage-bridge-proposer"),
            ProposalBatch {
                proposals: vec![Proposal::mutate(seed, 41).build()],
                semantics: ProposalBatchSemantics::Alternatives,
                metadata: MetadataBag::new(),
            },
            Cost::zero(),
        )
        .unwrap();
    let public_batch = format!("pb_{}", batch.batch_id.as_uuid());
    let script = python_worker(&format!(
        r#"
import json
import os
import sys

request = json.loads(sys.stdin.readline())
payload = request["params"]["payload"]

print(json.dumps({{
    "jsonrpc": "2.0",
    "id": "%s::apply" % payload["stage_call_id"],
    "method": "leaven/proposal.apply",
    "params": {{
        "schema_version": "leaven.plan.v1",
        "plan_id": "plan_runner_proposal_apply",
        "consistency": {{"kind": "latest_at_start"}},
        "mode": {{"kind": "execute"}},
        "ops": [{{
            "kind": "write",
            "name": "apply",
            "idempotency_key": "proposal-apply-stage-bridge-0001",
            "write": {{
                "kind": "apply_proposal_batch",
                "proposal_batch": "{public_batch}",
                "policy": "apply_first_valid"
            }},
        }}],
        "return": ["apply"],
        "commit": {{"kind": "graph_writes_atomic", "on_stale": "reject"}},
    }},
}}), flush=True)

reply = json.loads(sys.stdin.readline())
assert reply["result"]["method"] == "leaven/proposal.apply", reply
assert reply["result"]["capability_fingerprint"] == os.environ["LEAVEN_CAPABILITY_FINGERPRINT"], reply
assert reply["result"]["primary"]["kind"] == "apply_receipt", reply
assert reply["result"]["receipts"][0]["write_kind"] == "apply_proposal_batch", reply

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
            "summary": "proposal applied",
            "value": "applied",
            "visibility": "optimizer_visible",
            "data_classes": ["candidate.output"],
        }},
    }},
}}), flush=True)
"#
    ));
    let mut session = spawn(&script, package, profile);
    let host = RunContextGraphEffectHost::new(
        &mut context,
        [batch],
        "fp_cap_sha256_stage_bridge",
        "fp_policy_sha256_stage_bridge",
        "rev_stage_bridge_base",
        "rev_stage_bridge_final",
    );

    let response = session
        .dispatch_stage_run(&runner_request("proposal apply"), &host)
        .unwrap();

    assert_eq!(response.result().output().as_value()["value"], "applied");
    drop(host);
    assert_eq!(
        context.graph().candidate_count(),
        2,
        "worker callback must create the child through RunContext::apply_batch"
    );
    forget(script);
}

#[test]
fn dispatch_stage_run_services_worker_initiated_event_emit_through_runcontext() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let profile = acp_profile(&package);
    let mut graph = RunGraph::<BridgeProblem>::new(RunId::new());
    let mut budget = BudgetLedger::new(Budget::unlimited());
    let mut context = RunContext::<BridgeProblem>::new(&mut graph, &mut budget);
    let script = python_worker(
        r#"
import json
import os
import sys

request = json.loads(sys.stdin.readline())
payload = request["params"]["payload"]

print(json.dumps({
    "jsonrpc": "2.0",
    "id": "%s::event" % payload["stage_call_id"],
    "method": "leaven/event.emit",
    "params": {
        "schema_version": "leaven.plan.v1",
        "plan_id": "plan_runner_event_emit",
        "consistency": {"kind": "latest_at_start"},
        "mode": {"kind": "execute"},
        "ops": [{
            "kind": "write",
            "name": "status",
            "idempotency_key": "event-emit-stage-bridge-0001",
            "write": {
                "kind": "emit_run_event",
                "event_kind": "stage.bridge.checked",
                "payload_schema": "fp_schema_sha256_stage_bridge_event",
                "payload": {
                    "kind": "external_event",
                    "ok": True,
                    "stage_call_id": payload["stage_call_id"],
                },
                "visibility": "public"
            },
        }],
        "return": ["status"],
        "commit": {"kind": "graph_writes_atomic", "on_stale": "reject"},
    },
}), flush=True)

reply = json.loads(sys.stdin.readline())
assert reply["result"]["method"] == "leaven/event.emit", reply
assert reply["result"]["capability_fingerprint"] == os.environ["LEAVEN_CAPABILITY_FINGERPRINT"], reply
assert reply["result"]["primary"]["kind"] == "emit_run_event", reply
assert reply["result"]["receipts"][0]["write_kind"] == "emit_run_event", reply

print(json.dumps({
    "jsonrpc": "2.0",
    "id": request["id"],
    "result": {
        "schema_version": "leaven.stage_run.v1",
        "message": "stage_run_result",
        "stage": "runner",
        "stage_call_id": payload["stage_call_id"],
        "output": {
            "kind": "text",
            "summary": "event emitted",
            "value": "emitted",
            "visibility": "optimizer_visible",
            "data_classes": ["candidate.output"],
        },
    },
}), flush=True)
"#,
    );
    let mut session = spawn(&script, package, profile);
    let host = RunContextGraphEffectHost::new(
        &mut context,
        [],
        "fp_cap_sha256_stage_bridge",
        "fp_policy_sha256_stage_bridge",
        "rev_stage_bridge_base",
        "rev_stage_bridge_final",
    );

    let response = session
        .dispatch_stage_run(&runner_request("event emit"), &host)
        .unwrap();

    assert_eq!(response.result().output().as_value()["value"], "emitted");
    drop(host);
    let events = context.graph().events().collect::<Vec<_>>();
    assert_eq!(events.len(), 1);
    let RunEvent::ExternalEventEmitted {
        event_id,
        event_kind,
        payload_schema,
        payload,
        visibility,
    } = events[0]
    else {
        panic!("worker callback must record a RunContext external event");
    };
    assert_eq!(event_id, "event_status");
    assert_eq!(event_kind, "stage.bridge.checked");
    assert_eq!(payload_schema, "fp_schema_sha256_stage_bridge_event");
    assert!(payload.ok);
    assert_eq!(
        payload.stage_call_id.as_deref(),
        Some("sc_dispatch_contract")
    );
    assert_eq!(visibility, "public");
    forget(script);
}

#[test]
fn dispatch_stage_run_rejects_non_text_stage_output() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let profile = acp_profile(&package);
    // The worker answers the dispatch with a JSON-kind output, which the locked
    // stage-run result schema refuses for the V1 runner stage.
    let script = python_worker(
        r#"
import json
import sys

request = json.loads(sys.stdin.readline())
payload = request["params"]["payload"]
print(json.dumps({
    "jsonrpc": "2.0",
    "id": request["id"],
    "result": {
        "schema_version": "leaven.stage_run.v1",
        "message": "stage_run_result",
        "stage": "runner",
        "stage_call_id": payload["stage_call_id"],
        "output": {
            "kind": "json",
            "value": {"answer": "12"},
            "visibility": "optimizer_visible",
            "data_classes": ["candidate.output"],
        },
    },
}), flush=True)
"#,
    );
    let mut session = spawn(&script, package, profile);
    assert!(
        session
            .dispatch_stage_run(&runner_request("Expression: 5 + 7"), &RejectAllEffectHost)
            .is_err(),
        "a non-text runner stage output must be refused"
    );
    forget(script);
}

#[test]
fn dispatch_stage_run_refuses_target_material_in_the_runner_request() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let profile = acp_profile(&package);
    // No worker process is needed: the dispatch validates the request before it
    // ever reaches the worker, so target material is refused host-side.
    let script = python_worker("import sys\nsys.stdin.readline()\n");
    let mut session = spawn(&script, package, profile);
    let mut request = runner_request("Expression: 5 + 7");
    request["payload"]["case_input"]["case.target"] = json!("secret answer");
    assert!(
        session
            .dispatch_stage_run(&request, &RejectAllEffectHost)
            .is_err(),
        "target material must be refused before dispatch"
    );
    forget(script);
}

#[test]
fn runner_dispatch_rejects_text_output_without_a_value() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let profile = acp_profile(&package);
    // A text OutputRecord is schema-valid without a `value` field, but the runner
    // cannot interpret it as a candidate output and must refuse it.
    let script = python_worker(
        r#"
import json
import sys

request = json.loads(sys.stdin.readline())
payload = request["params"]["payload"]
print(json.dumps({
    "jsonrpc": "2.0",
    "id": request["id"],
    "result": {
        "schema_version": "leaven.stage_run.v1",
        "message": "stage_run_result",
        "stage": "runner",
        "stage_call_id": payload["stage_call_id"],
        "output": {
            "kind": "text",
            "summary": "no value",
            "visibility": "optimizer_visible",
            "data_classes": ["candidate.output"],
        },
    },
}), flush=True)
"#,
    );
    let mut session = spawn(&script, package, profile);
    let lm = MockArithmeticLm;
    let mut dispatch = RunnerDispatch::new(&lm, "value_gap");
    assert!(
        dispatch
            .run_rollout(
                session.session_mut(),
                "cand_value_gap",
                "case_value_gap",
                &json!({"question": "5 + 7", "prompt": "Expression: 5 + 7"}),
            )
            .is_err(),
        "a text stage output without a value must be refused"
    );
    forget(script);
}

#[test]
fn optimize_prompt_rejects_an_empty_case_set() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let profile = acp_profile(&package);
    // The worker is never reached: the loop refuses an empty case set up front.
    let script = python_worker("import sys\nsys.stdin.readline()\n");
    let mut session = spawn(&script, package, profile);
    let lm = MockArithmeticLm;
    let result = optimize_prompt(
        session.session_mut(),
        OptimizeConfig {
            lm: &lm,
            run_id: "empty".to_owned(),
            seed: PromptArtifact::new("seed"),
            cases: Vec::new(),
            minibatch: 1,
            reward: |_output, _target| 0.0,
            reflect: |_parent, _feedback| None,
            max_iterations: 1,
        },
    );
    assert!(result.is_err(), "an empty case set must be refused");
    forget(script);
}

fn runner_request(prompt: &str) -> Value {
    json!({
        "schema_version": "leaven.stage_run.v1",
        "message": "stage_run_request",
        "stage": "runner",
        "payload": {
            "schema_version": "leaven.stage_payloads.v1",
            "role": "runner",
            "run": "run_dispatch_contract",
            "stage_call_id": "sc_dispatch_contract",
            "candidate": "cand_dispatch_contract",
            "case": "case_dispatch_contract",
            "case_input": {"question": "5 + 7", "prompt": prompt},
            "target_forbidden": true,
            "capability_fingerprint": "fp_cap_sha256_dispatch_contract"
        }
    })
}

fn python_worker(body: &str) -> TempDir {
    let temp = TempDir::new().unwrap();
    fs::write(temp.path().join("worker.py"), body).unwrap();
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

struct BridgeEvidence;

impl Evidence for BridgeEvidence {}
