use leaven_core::{ExternalRef, InfoRef};
use leaven_gepa::{ReflectRequest, ReflectiveExample};

use crate::doctor::{DoctorError, WorkspaceFile, fixed_parent};

pub fn fixture_reflect_request() -> ReflectRequest<String> {
    ReflectRequest::new(fixed_parent(), "rust-test-debugging/SKILL.md")
        .with_examples([ReflectiveExample {
            input: "cargo nextest run -p leaven-gepa --test agent_stage_routing failed after the proposal stage emitted no workspace diff.".to_owned(),
            output: Some(
                "The agent suggested changing a prompt but never inspected the materialized skill."
                    .to_owned(),
            ),
            score: Some(0.25),
            feedback: "The reflector must inspect the current skill body and explain concrete workspace edits, not propose from metadata alone."
                .to_owned(),
            source_refs: vec![InfoRef::Candidate(fixed_parent())],
            ..ReflectiveExample::default()
        }])
        .with_source_refs([InfoRef::External(ExternalRef {
            kind: "doctor".to_owned(),
            id: "gepa-skill-bank-proposal-render".to_owned(),
        })])
        .with_attempt_index(0)
}

pub fn fixture_workspace_files() -> Result<Vec<WorkspaceFile>, DoctorError> {
    Ok(vec![
        WorkspaceFile::markdown(
            "selected skill",
            ".agents/skills/rust-test-debugging/SKILL.md",
        )?,
        WorkspaceFile::markdown(
            "supporting notes",
            ".agents/skills/rust-test-debugging/examples/failure.md",
        )?,
    ])
}
