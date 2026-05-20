use std::collections::BTreeMap;

use leaven_agentic_skill::SkillFileChangeKind;
use leaven_artifact_skill::{SkillBank, SkillFile, SkillFolder, SkillName, SkillPath};
use serde_json::json;
use trace2skill_spreadsheetbench::{
    Trace2SkillJsonPatchArtifact, Trace2SkillJsonPatchMergeBatch, Trace2SkillJsonPatchMergeInput,
    Trace2SkillJsonPatchMergeLevel, Trace2SkillJsonPatchReplayInput,
    replay_trace2skill_json_patch_merge,
};

#[test]
fn replays_json_patch_merge_tree_and_applies_selected_final_patch() {
    let (parent, skill) = spreadsheet_skill_bank();
    let accepted_leaf = fenced_json_patch(&json!({
        "reasoning": "Many row deletion failures share the same fix.",
        "edits": row_safety_edits(),
        "changelog_entries": ["Added row safety guidance"]
    }));
    let discarded_leaf = fenced_json_patch(&json!({
        "reasoning": "One task preferred a formatting reminder.",
        "edits": [
            {
                "file": "SKILL.md",
                "op": "add_section",
                "after_section": "## Important Requirements",
                "target_section": "## Formatting Preference",
                "content": "- Preserve the analyst's preferred row colors."
            }
        ],
        "changelog_entries": ["Added formatting preference"]
    }));
    let final_patch = fenced_json_patch(&json!({
        "reasoning": "Consolidated prevalent row deletion safety guidance.",
        "edits": row_safety_edits(),
        "changelog_entries": ["Merged row safety guidance"]
    }));

    let replay = replay_trace2skill_json_patch_merge(Trace2SkillJsonPatchReplayInput {
        parent: &parent,
        skill: &skill,
        leaf_patches: vec![
            Trace2SkillJsonPatchArtifact {
                plan_id: "map/error-13-1",
                payload: &accepted_leaf,
                support_count: 53,
            },
            Trace2SkillJsonPatchArtifact {
                plan_id: "map/error-59902",
                payload: &discarded_leaf,
                support_count: 1,
            },
        ],
        merge_levels: vec![Trace2SkillJsonPatchMergeLevel {
            batches: vec![Trace2SkillJsonPatchMergeBatch {
                output_plan_id: "merge/l0/b0",
                output_payload: &final_patch,
                support_count: 53,
                inputs: vec![
                    Trace2SkillJsonPatchMergeInput::Accepted {
                        plan_id: "map/error-13-1",
                    },
                    Trace2SkillJsonPatchMergeInput::Discarded {
                        plan_id: "map/error-59902",
                        reason: "single-task visual preference",
                    },
                ],
            }],
        }],
        final_plan_id: "merge/l0/b0",
    })
    .unwrap();

    assert_eq!(replay.merge_tree.leaf_plans().len(), 2);
    assert_eq!(replay.merge_tree.levels().len(), 1);
    assert_eq!(replay.merge_tree.final_plan_id().as_str(), "merge/l0/b0");
    assert_eq!(replay.merge_tree.final_plan().edits().len(), 2);
    let batch = &replay.merge_tree.levels()[0].batches()[0];
    assert_eq!(batch.accepted_input_ids()[0].as_str(), "map/error-13-1");
    assert_eq!(batch.discarded_input_ids()[0].as_str(), "map/error-59902");

    assert_eq!(
        replay.final_reasoning,
        "Consolidated prevalent row deletion safety guidance."
    );
    assert_eq!(
        replay.final_changelog_entries,
        ["Merged row safety guidance"]
    );
    let child = replay.application.child().get(&skill).unwrap();
    assert!(skill_file_text(child, "SKILL.md").contains("references/row-safety.md"));
    assert!(skill_file_text(child, "references/row-safety.md").contains("Delete rows"));
    assert!(
        replay
            .application
            .report()
            .files_changed
            .iter()
            .any(|file| {
                file.skill == skill
                    && file.path == SkillPath::new("references/row-safety.md").unwrap()
                    && file.kind == SkillFileChangeKind::Added
            })
    );
}

fn row_safety_edits() -> serde_json::Value {
    json!([
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
    ])
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
