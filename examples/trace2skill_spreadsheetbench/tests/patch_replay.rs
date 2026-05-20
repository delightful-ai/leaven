use std::{collections::BTreeMap, fs, path::Path};

use leaven_agentic_skill::SkillFileChangeKind;
use leaven_artifact_skill::{SkillBank, SkillFile, SkillFolder, SkillName, SkillPath};
use serde_json::json;
use trace2skill_spreadsheetbench::{
    Trace2SkillJsonPatchArtifact, Trace2SkillJsonPatchMergeBatch, Trace2SkillJsonPatchMergeInput,
    Trace2SkillJsonPatchMergeLevel, Trace2SkillJsonPatchReplayInput,
    Trace2SkillSavedJsonPatchReplayInput, replay_trace2skill_json_patch_merge,
    replay_trace2skill_saved_json_patch_outputs,
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
    assert_eq!(replay.applied_plan_id.as_str(), "merge/l0/b0");
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

#[test]
fn loads_upstream_saved_intermediates_and_prefers_translated_final_patch() {
    let (parent, skill) = spreadsheet_skill_bank();
    let tempdir = tempfile::tempdir().unwrap();
    let root = tempdir.path();
    fs::create_dir_all(root.join("map_patches")).unwrap();
    fs::create_dir_all(root.join("merge_level_1")).unwrap();
    write_patch(
        &root.join("map_patches/patch_0001.json"),
        &json_patch("Map row safety", row_safety_edits_with_body("map safety")),
    );
    write_patch(
        &root.join("map_patches/patch_0002.json"),
        &json_patch("Map format preference", format_preference_edits()),
    );
    write_patch(
        &root.join("merge_level_1/merged_0001.json"),
        &json_patch(
            "Merged row safety",
            row_safety_edits_with_body("merged safety"),
        ),
    );
    write_patch(
        &root.join("final_patch.json"),
        &json_patch(
            "Final pre-translation row safety",
            row_safety_edits_with_body("pre translation"),
        ),
    );
    write_patch(
        &root.join("translated_final_patch.json"),
        &json_patch(
            "Translated exact row safety",
            row_safety_edits_with_body("translated exact"),
        ),
    );

    let replay =
        replay_trace2skill_saved_json_patch_outputs(Trace2SkillSavedJsonPatchReplayInput {
            parent: &parent,
            skill: &skill,
            intermediates_dir: root,
            merge_batch_size: 2,
        })
        .unwrap();

    assert_eq!(replay.merge_tree.leaf_plans().len(), 2);
    assert_eq!(replay.merge_tree.levels().len(), 2);
    assert_eq!(
        replay.merge_tree.final_plan_id().as_str(),
        "final/final_patch"
    );
    assert_eq!(
        replay.applied_plan_id.as_str(),
        "final/translated_final_patch"
    );
    assert_eq!(replay.final_reasoning, "Translated exact row safety");
    let child = replay.application.child().get(&skill).unwrap();
    assert!(skill_file_text(child, "references/row-safety.md").contains("translated exact"));
}

fn row_safety_edits() -> serde_json::Value {
    row_safety_edits_with_body("Delete rows from bottom to top and stay inside the explicit range.")
}

fn row_safety_edits_with_body(body: &str) -> serde_json::Value {
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
            "content": format!("# Row Safety\n\n{body}\n")
        }
    ])
}

fn format_preference_edits() -> serde_json::Value {
    json!([
        {
            "file": "SKILL.md",
            "op": "add_section",
            "after_section": "## Important Requirements",
            "target_section": "## Formatting Preference",
            "content": "- Preserve analyst formatting preferences."
        }
    ])
}

fn json_patch(reasoning: &str, edits: impl Into<serde_json::Value>) -> serde_json::Value {
    let edits = edits.into();
    json!({
        "reasoning": reasoning,
        "edits": edits,
        "changelog_entries": [reasoning]
    })
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

fn write_patch(path: &Path, value: &serde_json::Value) {
    fs::write(path, serde_json::to_vec_pretty(value).unwrap()).unwrap();
}

fn skill_file_text(folder: &SkillFolder, path: &str) -> String {
    let path = SkillPath::new(path).unwrap();
    String::from_utf8(folder.file(&path).unwrap().bytes().to_vec()).unwrap()
}
