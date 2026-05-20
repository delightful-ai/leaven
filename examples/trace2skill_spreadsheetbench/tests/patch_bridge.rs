use std::collections::BTreeMap;

use leaven_agentic_skill::{SkillParsedPatchError, SkillPatchPlanError};
use leaven_artifact_skill::{SkillBank, SkillFile, SkillFolder, SkillName, SkillPath};
use serde_json::json;
use trace2skill_spreadsheetbench::{
    Trace2SkillPatchError, Trace2SkillPatchLoweringInput, apply_trace2skill_json_patch,
    lower_trace2skill_json_patch,
};

#[test]
fn trace2skill_json_patch_lowers_to_validated_plan_and_atomic_changes() {
    let (parent, skill) = spreadsheet_skill_bank();
    let payload = fenced_json_patch(&json!({
        "reasoning": "Structural row deletion failures recur across many traces.",
        "edits": [
            {
                "file": "SKILL.md",
                "op": "append_to_section",
                "target_section": "## Important Requirements",
                "content": "- Before deleting rows, read references/row-safety.md and verify the task range."
            },
            {
                "file": "references/row-safety.md",
                "op": "create",
                "content": "# Row Safety\n\nDelete rows from bottom to top and stay inside the explicit range.\n"
            }
        ],
        "changelog_entries": ["Added row deletion safety guidance"]
    }));

    let lowered = lower_trace2skill_json_patch(Trace2SkillPatchLoweringInput {
        parent: &parent,
        skill: &skill,
        payload: &payload,
        support_count: 53,
    })
    .unwrap();

    assert_eq!(
        lowered.reasoning,
        "Structural row deletion failures recur across many traces."
    );
    assert_eq!(lowered.plan.edits().len(), 2);
    assert!(
        lowered
            .plan
            .edits()
            .iter()
            .all(|edit| edit.support().count() == 53)
    );
    assert_eq!(
        lowered.changelog_entries,
        ["Added row deletion safety guidance"]
    );
    assert_eq!(lowered.changes.len(), 2);

    let application = apply_trace2skill_json_patch(Trace2SkillPatchLoweringInput {
        parent: &parent,
        skill: &skill,
        payload: &payload,
        support_count: 53,
    })
    .unwrap();

    assert_eq!(application.parent(), &parent);
    assert!(matches!(
        application.change(),
        leaven_artifact_skill::SkillBankChange::Atomic(changes) if changes.len() == 2
    ));
    let child = application.child().get(&skill).unwrap();
    assert!(skill_file_text(child, "SKILL.md").contains("references/row-safety.md"));
    assert!(skill_file_text(child, "references/row-safety.md").contains("Delete rows"));
}

#[test]
fn trace2skill_json_patch_rejects_unlinked_reference_creates() {
    let (parent, skill) = spreadsheet_skill_bank();
    let payload = fenced_json_patch(&json!({
        "reasoning": "Single low-support observation should be a reference.",
        "edits": [
            {
                "file": "references/orphan.md",
                "op": "create",
                "content": "# Orphan\n"
            }
        ],
        "changelog_entries": []
    }));

    let error = lower_trace2skill_json_patch(Trace2SkillPatchLoweringInput {
        parent: &parent,
        skill: &skill,
        payload: &payload,
        support_count: 1,
    })
    .unwrap_err();

    assert!(matches!(
        error,
        Trace2SkillPatchError::ParsedPatch(SkillParsedPatchError::Plan(
            SkillPatchPlanError::UnlinkedReferenceCreate { .. }
        ))
    ));
}

#[test]
fn trace2skill_json_patch_requires_translated_exact_section_targets() {
    let (parent, skill) = spreadsheet_skill_bank();
    let payload = fenced_json_patch(&json!({
        "reasoning": "The edit was not translated to the actual file heading.",
        "edits": [
            {
                "file": "SKILL.md",
                "op": "append_to_section",
                "target_section": "## Missing Heading",
                "content": "- This should not silently no-op."
            }
        ],
        "changelog_entries": []
    }));

    let error = lower_trace2skill_json_patch(Trace2SkillPatchLoweringInput {
        parent: &parent,
        skill: &skill,
        payload: &payload,
        support_count: 1,
    })
    .unwrap_err();

    assert!(matches!(
        error,
        Trace2SkillPatchError::SectionNotFound { section, .. } if section == "## Missing Heading"
    ));
}

fn spreadsheet_skill_bank() -> (SkillBank, SkillName) {
    let skill = SkillName::new("xlsx").unwrap();
    let mut entries = BTreeMap::new();
    entries.insert(
        SkillPath::skill_md(),
        SkillFile::text(
            "---\n\
             name: xlsx\n\
             description: Use when manipulating spreadsheets with local tools.\n\
             ---\n\
             # Spreadsheet Skill\n\
             \n\
             ## Important Requirements\n\
             - Preserve formulas and verify outputs.\n",
        ),
    );
    let folder = SkillFolder::from_entries(skill.clone(), entries).unwrap();
    (SkillBank::from_folders([folder]).unwrap(), skill)
}

fn fenced_json_patch(value: &serde_json::Value) -> String {
    format!("Trace2Skill response:\n```json\n{value}\n```\n")
}

fn skill_file_text(folder: &SkillFolder, path: &str) -> String {
    let path = SkillPath::new(path).unwrap();
    String::from_utf8(folder.file(&path).unwrap().bytes().to_vec()).unwrap()
}
