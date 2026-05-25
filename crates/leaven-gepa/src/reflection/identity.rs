use leaven_core::InfoRef;
use leaven_evidence::Attachment;
use leaven_kernel::{AgentId, CaseRunId};
use uuid::Uuid;

use super::{Checks, ReflectiveCase, ReflectiveRun, ReflectiveSideInfoValue, ReflectiveValue};

pub(super) fn deterministic_example_run_id(
    input: &ReflectiveValue,
    expected: Option<&ReflectiveValue>,
    produced: Option<&ReflectiveValue>,
    score: Option<f64>,
    feedback: &str,
) -> CaseRunId {
    deterministic_run_id_from_parts(&DeterministicRunIdParts {
        input,
        expected,
        agent_id: None,
        attempt_index: Some(0),
        produced,
        score,
        max_score: None,
        passed: None,
        feedback,
        checks: None,
        side_info: &[],
        attachments: &[],
        source_refs: &[],
    })
}

pub(super) fn refresh_default_run_id(case: &mut ReflectiveCase) {
    for run in &mut case.runs {
        run.run_id = deterministic_run_id(&case.input, case.expected.as_ref(), run);
    }
}

struct DeterministicRunIdParts<'a> {
    input: &'a ReflectiveValue,
    expected: Option<&'a ReflectiveValue>,
    agent_id: Option<&'a AgentId>,
    attempt_index: Option<usize>,
    produced: Option<&'a ReflectiveValue>,
    score: Option<f64>,
    max_score: Option<f64>,
    passed: Option<bool>,
    feedback: &'a str,
    checks: Option<&'a Checks>,
    side_info: &'a [(String, ReflectiveSideInfoValue)],
    attachments: &'a [Attachment],
    source_refs: &'a [InfoRef],
}

fn deterministic_run_id(
    input: &ReflectiveValue,
    expected: Option<&ReflectiveValue>,
    run: &ReflectiveRun,
) -> CaseRunId {
    deterministic_run_id_from_parts(&DeterministicRunIdParts {
        input,
        expected,
        agent_id: run.agent_id.as_ref(),
        attempt_index: run.attempt_index,
        produced: run.produced.as_ref(),
        score: run.score,
        max_score: run.max_score,
        passed: run.passed,
        feedback: &run.feedback,
        checks: run.checks.as_ref(),
        side_info: &run.side_info,
        attachments: &run.attachments,
        source_refs: &run.source_refs,
    })
}

fn deterministic_run_id_from_parts(parts: &DeterministicRunIdParts<'_>) -> CaseRunId {
    let mut hash = 0x6c62_272e_07bb_0142_62b8_2175_6295_c58d_u128;
    feed_hash(&mut hash, b"leaven.gepa.reflective_run.v2");
    feed_json(&mut hash, parts.input);
    feed_json(&mut hash, &parts.expected);
    feed_json(&mut hash, &parts.agent_id);
    feed_json(&mut hash, &parts.attempt_index);
    feed_json(&mut hash, &parts.produced);
    feed_optional_f64(&mut hash, b"score", parts.score);
    feed_optional_f64(&mut hash, b"max_score", parts.max_score);
    feed_json(&mut hash, &parts.passed);
    feed_hash(&mut hash, b"feedback");
    feed_hash(&mut hash, parts.feedback.as_bytes());
    feed_json(&mut hash, &parts.checks);
    feed_json(&mut hash, &parts.side_info);
    feed_json(&mut hash, &parts.attachments);
    feed_json(&mut hash, &parts.source_refs);
    CaseRunId::from_uuid(Uuid::from_u128(hash))
}

fn feed_json<T: serde::Serialize>(hash: &mut u128, value: &T) {
    if let Ok(bytes) = serde_json::to_vec(value) {
        feed_hash(hash, &bytes);
    }
    feed_hash(hash, b"\0");
}

fn feed_optional_f64(hash: &mut u128, label: &[u8], value: Option<f64>) {
    feed_hash(hash, label);
    match value {
        Some(value) => {
            feed_hash(hash, b":some");
            feed_hash(hash, &value.to_bits().to_le_bytes());
        }
        None => feed_hash(hash, b":none"),
    }
}

fn feed_hash(hash: &mut u128, bytes: &[u8]) {
    for byte in bytes {
        *hash ^= u128::from(*byte);
        *hash = hash.wrapping_mul(0x0000_0000_0100_0000_0000_0000_013b_u128);
    }
}
