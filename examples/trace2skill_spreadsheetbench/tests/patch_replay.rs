use std::{collections::BTreeMap, fs, path::Path};

use leaven_agentic_skill::SkillFileChangeKind;
use leaven_artifact_skill::{SkillBank, SkillFile, SkillFolder, SkillName, SkillPath};
use leaven_evidence::{
    AgentAnalystCallEvidence, AgentAnalystCallEvidenceInput, AgentAnalystCallStatus,
    AgentAnalystFanoutEvidence, AgentAnalystRole, OutputRecord,
};
use serde_json::json;
use trace2skill_spreadsheetbench::{
    import_trace2skill_saved_map_patches_into_fanout, replay_trace2skill_json_patch_merge,
    replay_trace2skill_saved_json_patch_outputs, Trace2SkillJsonPatchArtifact,
    Trace2SkillJsonPatchMergeBatch, Trace2SkillJsonPatchMergeInput, Trace2SkillJsonPatchMergeLevel,
    Trace2SkillJsonPatchReplayInput, Trace2SkillSavedJsonPatchReplayInput,
    Trace2SkillSavedMapPatchFanoutInput,
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
    assert!(replay
        .application
        .report()
        .files_changed
        .iter()
        .any(|file| {
            file.skill == skill
                && file.path == SkillPath::new("references/row-safety.md").unwrap()
                && file.kind == SkillFileChangeKind::Added
        }));
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

#[test]
fn imports_saved_map_patches_into_analyst_fanout_by_batch_index() {
    let (parent, skill) = spreadsheet_skill_bank();
    let mut fanout =
        AgentAnalystFanoutEvidence::new(["error-13-1-1".to_owned(), "success-14-1-2".to_owned()])
            .unwrap();
    fanout
        .push(pending_call(
            "error-13-1-1",
            AgentAnalystRole::Error,
            "13-1",
            "error prompt",
        ))
        .unwrap();
    fanout
        .push(pending_call(
            "success-14-1-2",
            AgentAnalystRole::Success,
            "14-1",
            "success prompt",
        ))
        .unwrap();
    let tempdir = tempfile::tempdir().unwrap();
    let root = tempdir.path();
    fs::create_dir_all(root.join("map_patches")).unwrap();
    write_patch(
        &root.join("map_patches/patch_0001.json"),
        &json_patch_with_batch("Success analyst patch", format_preference_edits(), 2),
    );
    write_patch(
        &root.join("map_patches/patch_0002.json"),
        &json_patch_with_batch("Error analyst patch", row_safety_edits(), 1),
    );

    let imported =
        import_trace2skill_saved_map_patches_into_fanout(Trace2SkillSavedMapPatchFanoutInput {
            parent: &parent,
            skill: &skill,
            fanout: &fanout,
            intermediates_dir: root,
            parse_failure_dir: None,
        })
        .unwrap();

    assert_eq!(
        imported.completed_call_ids(),
        vec!["error-13-1-1", "success-14-1-2"]
    );
    assert!(imported.pending_call_ids().is_empty());
    let error = imported.by_call("error-13-1-1").unwrap();
    assert_eq!(error.role(), AgentAnalystRole::Error);
    assert_eq!(error.source_task_ids(), ["13-1"]);
    assert!(matches!(error.prompt(), OutputRecord::Inline { text, .. } if text == "error prompt"));
    assert!(matches!(error.status(), AgentAnalystCallStatus::Succeeded));
    assert!(
        matches!(error.response(), Some(OutputRecord::BlobRef(reference)) if reference.key.ends_with("map_patches/patch_0002.json"))
    );
    let success = imported.by_call("success-14-1-2").unwrap();
    assert!(matches!(
        success.status(),
        AgentAnalystCallStatus::Succeeded
    ));
    assert!(
        matches!(success.response(), Some(OutputRecord::BlobRef(reference)) if reference.key.ends_with("map_patches/patch_0001.json"))
    );
}

#[test]
fn imports_saved_map_parse_failures_without_completing_unsaved_calls() {
    let (parent, skill) = spreadsheet_skill_bank();
    let mut fanout = AgentAnalystFanoutEvidence::new([
        "error-13-1-1".to_owned(),
        "success-14-1-2".to_owned(),
        "error-59902-3".to_owned(),
    ])
    .unwrap();
    fanout
        .push(pending_call(
            "error-13-1-1",
            AgentAnalystRole::Error,
            "13-1",
            "error prompt",
        ))
        .unwrap();
    fanout
        .push(pending_call(
            "success-14-1-2",
            AgentAnalystRole::Success,
            "14-1",
            "success prompt",
        ))
        .unwrap();
    fanout
        .push(pending_call(
            "error-59902-3",
            AgentAnalystRole::Error,
            "59902",
            "unsaved prompt",
        ))
        .unwrap();

    let tempdir = tempfile::tempdir().unwrap();
    let root = tempdir.path();
    fs::create_dir_all(root.join("map_patches")).unwrap();
    write_patch(
        &root.join("map_patches/patch_0001.json"),
        &json_patch_with_batch("Success analyst patch", format_preference_edits(), 2),
    );
    let parse_failure_dir = root.join("parse_failures_parallel");
    let parse_failure_path =
        parse_failure_dir.join("map/20260520_010203_000000_batch_0001_json_parse_failed.md");
    write_parse_failure(&parse_failure_path, "map", "batch_0001", "json-fence");

    let imported =
        import_trace2skill_saved_map_patches_into_fanout(Trace2SkillSavedMapPatchFanoutInput {
            parent: &parent,
            skill: &skill,
            fanout: &fanout,
            intermediates_dir: root,
            parse_failure_dir: Some(&parse_failure_dir),
        })
        .unwrap();

    assert_eq!(
        imported.completed_call_ids(),
        vec!["error-13-1-1", "success-14-1-2"]
    );
    assert_eq!(imported.pending_call_ids(), vec!["error-59902-3"]);
    let failed = imported.by_call("error-13-1-1").unwrap();
    assert_eq!(failed.role(), AgentAnalystRole::Error);
    assert_eq!(failed.source_task_ids(), ["13-1"]);
    assert!(matches!(failed.prompt(), OutputRecord::Inline { text, .. } if text == "error prompt"));
    assert!(
        matches!(failed.response(), Some(OutputRecord::BlobRef(reference)) if reference.key.ends_with("parse_failures_parallel/map/20260520_010203_000000_batch_0001_json_parse_failed.md"))
    );
    assert!(
        matches!(
            failed.status(),
            AgentAnalystCallStatus::ParseFailed { reason, artifact: Some(OutputRecord::BlobRef(reference)) }
                if reason == "upstream Trace2Skill map batch_0001 failed json-fence parsing"
                    && reference.key.ends_with("parse_failures_parallel/map/20260520_010203_000000_batch_0001_json_parse_failed.md")
        ),
        "saved upstream parse-failure markdown should make the batch terminal"
    );

    let success = imported.by_call("success-14-1-2").unwrap();
    assert!(matches!(
        success.status(),
        AgentAnalystCallStatus::Succeeded
    ));
    assert!(
        matches!(success.response(), Some(OutputRecord::BlobRef(reference)) if reference.key.ends_with("map_patches/patch_0001.json"))
    );
    assert!(matches!(
        imported.by_call("error-59902-3").unwrap().status(),
        AgentAnalystCallStatus::Pending
    ));
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

fn json_patch_with_batch(
    reasoning: &str,
    edits: impl Into<serde_json::Value>,
    batch_index: u32,
) -> serde_json::Value {
    let mut patch = json_patch(reasoning, edits);
    patch["batch_index"] = json!(batch_index);
    patch
}

fn pending_call(
    call_id: &str,
    role: AgentAnalystRole,
    source_task_id: &str,
    prompt: &str,
) -> AgentAnalystCallEvidence {
    AgentAnalystCallEvidence::new(AgentAnalystCallEvidenceInput {
        call_id: call_id.to_owned(),
        role,
        source_task_ids: vec![source_task_id.to_owned()],
        prompt: OutputRecord::inline(prompt),
        response: None,
        status: AgentAnalystCallStatus::Pending,
        retry_count: 0,
        support_count: 1,
    })
    .unwrap()
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

fn write_parse_failure(path: &Path, phase: &str, label: &str, expected_format: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(
        path,
        format!(
            "===== PARSE FAILURE START =====\n\
             PHASE: {phase}\n\
             LABEL: {label}\n\
             EXPECTED FORMAT: {expected_format}\n\
             \n\
             ===== USER MESSAGE 1 START =====\n\
             analyst prompt\n\
             ===== USER MESSAGE 1 END =====\n\
             \n\
             ===== FINAL RAW LLM RESPONSE START =====\n\
             no fenced patch here\n\
             ===== FINAL RAW LLM RESPONSE END =====\n\
             \n\
             ===== PARSE FAILURE END =====\n"
        ),
    )
    .unwrap();
}

fn skill_file_text(folder: &SkillFolder, path: &str) -> String {
    let path = SkillPath::new(path).unwrap();
    String::from_utf8(folder.file(&path).unwrap().bytes().to_vec()).unwrap()
}
