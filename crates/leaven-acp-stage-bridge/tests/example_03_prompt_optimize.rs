//! Example 03 end-to-end: optimize a prompt for arithmetic QA over the live ACP
//! bidirectional seam, exact-match scored, with a deterministic mock host LM.
//!
//! This is the first product-proof of the SDK bidirectional seam for the
//! prompt/LM/exact-match path. Every score comes from a real rollout: the host
//! dispatches `leaven/stage.run` to the Python worker, the worker calls
//! `leaven/lm.complete` BACK over the seam, the host's mock LM answers, and the
//! worker returns the output. A tiny but real GEPA-shaped accept loop screens a
//! reflected child against the seed on those live scores and accepts the strict
//! improvement, producing an `Optimized` `PromptArtifact`.
//!
//! It is not a proof of the reward vector, agent rollout, sandbox, live LM, or
//! `objective != instance`. The LM is a deterministic fixture; the seam,
//! dispatch, and accept loop are real.

use std::path::{Path, PathBuf};

use leaven_acp::{AcpProcessCommand, AcpStdioProcessSession};
use leaven_acp_stage_bridge::{
    CaseFeedback, MockArithmeticLm, OptCase, OptimizeConfig, PromptArtifact, optimize_prompt,
};
use leaven_public_seam::{AcpProfileDocument, PublicSeamPackage};
use serde_json::{Value, json};

#[test]
fn example_03_optimizes_a_prompt_over_the_live_bidirectional_seam() {
    let package = PublicSeamPackage::active_from_repo(workspace_root()).unwrap();
    let profile = acp_profile(&package);
    let mut session = spawn_runner_worker(package, profile);

    let lm = MockArithmeticLm;
    let cases = arithmetic_cases();
    let config = OptimizeConfig {
        lm: &lm,
        run_id: "example03".to_owned(),
        // The seed never surfaces the question to the model, so the mock LM has
        // nothing to evaluate and the seed scores zero — real headroom to improve.
        seed: PromptArtifact::new("You are a calculator. Always answer 0."),
        cases: cases.clone(),
        minibatch: 4,
        reward: exact_match,
        reflect: surface_question,
        max_iterations: 2,
    };

    let optimized =
        optimize_prompt(session.session_mut(), config).expect("optimization runs end to end");

    println!("example_03_ok=true");
    println!("cases={}", cases.len());
    println!("iterations={}", optimized.iterations);
    println!("candidates_evaluated={}", optimized.frontier.len());
    for candidate in &optimized.frontier {
        println!(
            "candidate id={} parent={:?} score={:.3} template={:?}",
            candidate.id,
            candidate.parent_id,
            candidate.score,
            candidate.artifact.template()
        );
    }
    println!(
        "best id={} score={:.3} template={:?}",
        optimized.best.id,
        optimized.best.score,
        optimized.best.artifact.template()
    );

    // A real improvement was accepted: the seed scored zero, the child scores
    // perfectly because the reflector surfaced the arithmetic question to the LM.
    let seed = optimized
        .frontier
        .iter()
        .find(|candidate| candidate.parent_id.is_none())
        .expect("seed candidate is on the frontier");
    assert!(
        seed.score.abs() < f64::EPSILON,
        "seed must not surface the question to the LM (score {})",
        seed.score
    );
    assert!(
        optimized.best.parent_id.is_some(),
        "best must be a reflected child, not the seed"
    );
    assert!(
        (optimized.best.score - 1.0).abs() < f64::EPSILON,
        "the accepted child solves every arithmetic case exactly (score {})",
        optimized.best.score
    );
    assert!(
        optimized.best.score > seed.score,
        "GEPA accepts only a strict improvement"
    );
    assert!(optimized.iterations >= 1, "at least one reflect/accept ran");

    // The produced artifact is a real, fully-typed PromptArtifact (not a stub).
    assert!(
        optimized.best.artifact.template().contains("{question}"),
        "the optimized prompt surfaces the question: {:?}",
        optimized.best.artifact.template()
    );
}

/// Exact-match reward: the candidate output must equal the scorer-only answer.
fn exact_match(output: &str, target: &Value) -> f64 {
    let answer = target.get("answer").and_then(Value::as_str).unwrap_or("");
    if output == answer { 1.0 } else { 0.0 }
}

/// A tiny but real reflector: reads the parent's per-case evidence and, when the
/// parent never surfaced the arithmetic question to the model (every output is
/// empty), proposes a prompt that does. The repair is derived from the feedback,
/// not a fixed edit applied unconditionally.
fn surface_question(parent: &PromptArtifact, feedback: &[CaseFeedback]) -> Option<PromptArtifact> {
    let parent_surfaces_question = parent.template().contains("{question}");
    let every_output_empty = feedback.iter().all(|item| item.output.trim().is_empty());
    if parent_surfaces_question || !every_output_empty {
        // Either the parent already surfaces the question or it is producing real
        // outputs; this tiny reflector has no further repair to offer.
        return None;
    }
    Some(PromptArtifact::new(
        "Compute the arithmetic expression and answer with only the integer.\n\
         Expression: {question}\nAnswer:",
    ))
}

fn arithmetic_cases() -> Vec<OptCase> {
    let fixture = workspace_root().join("docs/specs/leaven_py/examples/fixtures/arithmetic.jsonl");
    let text = std::fs::read_to_string(&fixture)
        .unwrap_or_else(|error| panic!("read fixture {}: {error}", fixture.display()));
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let row: Value = serde_json::from_str(line).expect("fixture row is JSON");
            let raw_id = row["id"].as_str().expect("fixture row has id");
            OptCase {
                // The wire CaseId pattern is `^case_[A-Za-z0-9_.:-]+$`.
                case_id: format!("case_{}", raw_id.replace('-', "_")),
                input: row["input"].clone(),
                target: row["target"].clone(),
            }
        })
        .collect()
}

fn spawn_runner_worker(
    package: PublicSeamPackage,
    profile: AcpProfileDocument,
) -> AcpStdioProcessSession {
    let worker = Path::new(env!("CARGO_MANIFEST_DIR")).join("worker/serve_stage_runner.py");
    AcpStdioProcessSession::spawn(
        package,
        profile,
        AcpProcessCommand::new("python3").arg(worker.to_string_lossy()),
        "secret-token",
        "stdio://worker/session",
        "fp_cap_sha256_stage_bridge",
    )
    .expect("spawn runner worker")
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
        ("leaven/human.review", "human.review"),
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
        .expect("crate lives under workspace/crates/leaven-acp-stage-bridge")
        .to_path_buf()
}
