use std::collections::BTreeMap;

use leaven_agentic_skill::SkillWorkspaceLayout;
use leaven_artifact_skill::{SkillBank, SkillFile, SkillFolder, SkillName, SkillPath};
use leaven_core::{ExternalRef, InfoRef};
use leaven_gepa::{ReflectRequest, ReflectiveCase, ReflectiveValue};
use leaven_gepa_agentic_skill::SkillBankReflectionInput;
use leaven_kernel::CandidateId;

pub fn fixture_reflection_input() -> SkillBankReflectionInput<String> {
    let request = ReflectRequest::new(fixed_parent(), "rust-test-debugging/SKILL.md")
        .with_examples([{
            let mut case = ReflectiveCase::from_example(
                ReflectiveValue::Text("cargo test -p leaven-gepa-agentic-skill --test skill_reflection failed after the proposal stage emitted no workspace diff.".to_owned()),
                None,
                Some(ReflectiveValue::Text(
                "The agent suggested changing a prompt but never inspected the materialized skill."
                    .to_owned(),
            )),
                Some(0.25),
                "The reflector must inspect the current skill body and explain concrete workspace edits, not propose from metadata alone.",
            );
            case.source_refs.push(InfoRef::Candidate(fixed_parent()));
            case
        }])
        .with_source_refs([InfoRef::External(ExternalRef {
            kind: "doctor".to_owned(),
            id: "gepa-skill-bank-proposal-render".to_owned(),
        })])
        .with_attempt_index(0);
    SkillBankReflectionInput::from_request(fixture_skill_bank(), request)
}

pub fn fixture_workspace_layout()
-> Result<SkillWorkspaceLayout, leaven_workspace::WorkspacePathError> {
    SkillWorkspaceLayout::new(".agents/skills")
}

fn fixture_skill_bank() -> SkillBank {
    let name = SkillName::new("rust-test-debugging").unwrap();
    let mut entries = BTreeMap::new();
    entries.insert(
        SkillPath::skill_md(),
        SkillFile::text(skill_md(
            "rust-test-debugging",
            "Debugs Rust tests. Use when Rust test failures need diagnosis.",
            "Read the failing test output, inspect the narrow code path, and patch it.",
        )),
    );
    entries.insert(
        SkillPath::new("examples/failure.md").unwrap(),
        SkillFile::text("cargo test failed before the agent inspected the skill body.\n"),
    );
    SkillBank::from_folders([SkillFolder::from_entries(name, entries).unwrap()]).unwrap()
}

fn skill_md(name: &str, description: &str, body: &str) -> String {
    format!("---\nname: {name}\ndescription: {description}\n---\n{body}\n")
}

fn fixed_parent() -> CandidateId {
    CandidateId::from_uuid(uuid::uuid!("00000000-0000-0000-0000-000000000001"))
}
