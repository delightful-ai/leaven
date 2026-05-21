use std::collections::BTreeMap;

use leaven_agentic_skill::{SkillParsedPatchError, SkillPatchPlanError};
use leaven_artifact_skill::{
    SkillBank, SkillBankChange, SkillFile, SkillFilePermissions, SkillFolder, SkillName, SkillPath,
};
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
fn trace2skill_json_patch_validates_reference_changes_against_full_skill_links() {
    let skill = SkillName::new("xlsx").unwrap();
    let mut entries = BTreeMap::new();
    entries.insert(
        SkillPath::skill_md(),
        SkillFile::with_permissions(
            b"---\nname: xlsx\ndescription: spreadsheet\n---\n# Skill\n\n## Important Requirements\n- Read references/row-safety.md before deleting rows.\n".to_vec(),
            SkillFilePermissions { executable: true },
        ),
    );
    entries.insert(
        SkillPath::new("references/row-safety.md").unwrap(),
        SkillFile::text("# Row Safety\n"),
    );
    let parent =
        SkillBank::from_folders([SkillFolder::from_entries(skill.clone(), entries).unwrap()])
            .unwrap();
    let payload = fenced_json_patch(&json!({
        "reasoning": "Deleting a still-linked reference would leave a dangling link.",
        "edits": [
            {
                "file": "references/row-safety.md",
                "op": "delete_file"
            }
        ],
        "changelog_entries": []
    }));

    let error = lower_trace2skill_json_patch(Trace2SkillPatchLoweringInput {
        parent: &parent,
        skill: &skill,
        payload: &payload,
        support_count: 2,
    })
    .unwrap_err();

    assert!(matches!(
        error,
        Trace2SkillPatchError::ParsedPatch(SkillParsedPatchError::Plan(
            SkillPatchPlanError::LinkedReferenceDeleted { .. }
        ))
    ));

    let mut entries = BTreeMap::new();
    entries.insert(
        SkillPath::skill_md(),
        SkillFile::with_permissions(
            b"---\nname: xlsx\ndescription: spreadsheet\n---\n# Skill\n\n## Important Requirements\n- Read references/row-safety.md before deleting rows.\n".to_vec(),
            SkillFilePermissions { executable: true },
        ),
    );
    let parent =
        SkillBank::from_folders([SkillFolder::from_entries(skill.clone(), entries).unwrap()])
            .unwrap();
    let payload = fenced_json_patch(&json!({
        "reasoning": "Creating a reference can repair an existing dangling skill link.",
        "edits": [
            {
                "file": "references/row-safety.md",
                "op": "create",
                "content": "# Row Safety\n"
            }
        ],
        "changelog_entries": []
    }));

    let lowered = lower_trace2skill_json_patch(Trace2SkillPatchLoweringInput {
        parent: &parent,
        skill: &skill,
        payload: &payload,
        support_count: 2,
    })
    .unwrap();

    assert_eq!(lowered.plan.edits().len(), 2);
    assert!(
        lowered.changes.iter().any(|change| matches!(
            change,
            SkillBankChange::WriteFile { path, file, .. }
                if path.is_skill_md() && file.permissions().executable
        )),
        "synthetic SKILL.md validation edit should preserve parent permissions"
    );

    let application = apply_trace2skill_json_patch(Trace2SkillPatchLoweringInput {
        parent: &parent,
        skill: &skill,
        payload: &payload,
        support_count: 2,
    })
    .unwrap();
    assert!(
        application
            .child()
            .get(&skill)
            .unwrap()
            .file(&SkillPath::skill_md())
            .unwrap()
            .permissions()
            .executable,
        "reference-only updates should not clear parent SKILL.md permissions"
    );
}

#[test]
fn trace2skill_json_patch_skill_edits_ignore_preexisting_dangling_reference_links() {
    let skill = SkillName::new("xlsx").unwrap();
    let mut entries = BTreeMap::new();
    entries.insert(
        SkillPath::skill_md(),
        SkillFile::text(
            "---\n\
             name: xlsx\n\
             description: spreadsheet\n\
             ---\n\
             # Skill\n\
             \n\
             ## Important Requirements\n\
             - Read references/missing.md when the legacy note is restored.\n\
             - Preserve formulas and verify outputs.\n",
        ),
    );
    let parent =
        SkillBank::from_folders([SkillFolder::from_entries(skill.clone(), entries).unwrap()])
            .unwrap();
    let payload = fenced_json_patch(&json!({
        "reasoning": "Unrelated text edits should not repair the whole legacy bank.",
        "edits": [
            {
                "file": "SKILL.md",
                "op": "append_to_section",
                "target_section": "## Important Requirements",
                "content": "- Keep row ranges explicit."
            }
        ],
        "changelog_entries": []
    }));

    let lowered = lower_trace2skill_json_patch(Trace2SkillPatchLoweringInput {
        parent: &parent,
        skill: &skill,
        payload: &payload,
        support_count: 2,
    })
    .unwrap();

    assert_eq!(lowered.plan.edits().len(), 1);
    let application = apply_trace2skill_json_patch(Trace2SkillPatchLoweringInput {
        parent: &parent,
        skill: &skill,
        payload: &payload,
        support_count: 2,
    })
    .unwrap();
    assert!(
        skill_file_text(application.child().get(&skill).unwrap(), "SKILL.md")
            .contains("Keep row ranges explicit.")
    );
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

#[test]
fn trace2skill_json_patch_accepts_embedded_markdown_fences_inside_json_strings() {
    let (parent, skill) = spreadsheet_skill_bank();
    let payload = "Trace2Skill response:\n```json\n{\"reasoning\":\"adds example\",\"edits\":[{\"file\":\"SKILL.md\",\"op\":\"append_to_section\",\"target_section\":\"## Important Requirements\",\"content\":\"- Use fenced examples:\\n```python\\nprint(1)\\n```\"}],\"changelog_entries\":[]}\n```\n";

    let lowered = lower_trace2skill_json_patch(Trace2SkillPatchLoweringInput {
        parent: &parent,
        skill: &skill,
        payload,
        support_count: 2,
    })
    .unwrap();

    assert_eq!(lowered.plan.edits().len(), 1);
}

#[test]
fn trace2skill_json_patch_rejects_add_section_with_missing_after_anchor() {
    let (parent, skill) = spreadsheet_skill_bank();
    let payload = fenced_json_patch(&json!({
        "reasoning": "The insertion anchor must be exact.",
        "edits": [
            {
                "file": "SKILL.md",
                "op": "add_section",
                "target_section": "## Late Guidance",
                "after_section": "## Missing Heading",
                "content": "- This should not be appended silently."
            }
        ],
        "changelog_entries": []
    }));

    let error = lower_trace2skill_json_patch(Trace2SkillPatchLoweringInput {
        parent: &parent,
        skill: &skill,
        payload: &payload,
        support_count: 2,
    })
    .unwrap_err();

    assert!(matches!(
        error,
        Trace2SkillPatchError::SectionNotFound { section, .. } if section == "## Missing Heading"
    ));
}

#[test]
fn trace2skill_json_patch_parses_raw_json_before_looking_for_fences() {
    let (parent, skill) = spreadsheet_skill_bank();
    let payload = json!({
        "reasoning": "Raw JSON string content mentions ```json but is not fenced.",
        "edits": [
            {
                "file": "SKILL.md",
                "op": "append_to_section",
                "target_section": "## Important Requirements",
                "content": "- Literal marker text: ```json"
            }
        ],
        "changelog_entries": []
    })
    .to_string();

    let lowered = lower_trace2skill_json_patch(Trace2SkillPatchLoweringInput {
        parent: &parent,
        skill: &skill,
        payload: &payload,
        support_count: 2,
    })
    .unwrap();

    assert_eq!(lowered.plan.edits().len(), 1);
}

#[test]
fn trace2skill_json_patch_accepts_common_json_fence_variants() {
    let (parent, skill) = spreadsheet_skill_bank();
    let payload = format!(
        "Trace2Skill response:\n``` JSON\n{}\n```\n",
        json!({
            "reasoning": "upper-case fence tag",
            "edits": [
                {
                    "file": "SKILL.md",
                    "op": "append_to_section",
                    "target_section": "## Important Requirements",
                    "content": "- Fence tag variants should parse."
                }
            ],
            "changelog_entries": []
        })
    );

    let lowered = lower_trace2skill_json_patch(Trace2SkillPatchLoweringInput {
        parent: &parent,
        skill: &skill,
        payload: &payload,
        support_count: 2,
    })
    .unwrap();

    assert_eq!(lowered.plan.edits().len(), 1);
}

#[test]
fn trace2skill_json_patch_selects_parseable_fence_and_accepts_single_line_fence() {
    let (parent, skill) = spreadsheet_skill_bank();
    let valid = json!({
        "reasoning": "second fence is the patch",
        "edits": [
            {
                "file": "SKILL.md",
                "op": "append_to_section",
                "target_section": "## Important Requirements",
                "content": "- Parse the valid JSON patch fence."
            }
        ],
        "changelog_entries": []
    });
    let payload = format!("```json\nnot json\n```\n```json {valid} ```");

    let lowered = lower_trace2skill_json_patch(Trace2SkillPatchLoweringInput {
        parent: &parent,
        skill: &skill,
        payload: &payload,
        support_count: 2,
    })
    .unwrap();

    assert_eq!(lowered.plan.edits().len(), 1);
}

#[test]
fn trace2skill_json_patch_ignores_non_patch_json_fences_and_indented_closers() {
    let (parent, skill) = spreadsheet_skill_bank();
    let valid = json!({
        "reasoning": "only the edits-bearing fence is a patch",
        "edits": [
            {
                "file": "SKILL.md",
                "op": "append_to_section",
                "target_section": "## Important Requirements",
                "content": "- Ignore unrelated JSON examples."
            }
        ],
        "changelog_entries": []
    });
    let payload = format!(
        "```text\nliteral ```json inside non-json block\n```\n```json\n{{\"example\":true}}\n```\n~~~~json\n{valid}\n   ~~~~\n"
    );

    let lowered = lower_trace2skill_json_patch(Trace2SkillPatchLoweringInput {
        parent: &parent,
        skill: &skill,
        payload: &payload,
        support_count: 2,
    })
    .unwrap();

    assert_eq!(lowered.plan.edits().len(), 1);
}

#[test]
fn trace2skill_json_patch_accepts_inline_close_without_preceding_space() {
    let (parent, skill) = spreadsheet_skill_bank();
    let valid = json!({
        "reasoning": "single line close",
        "edits": [
            {
                "file": "SKILL.md",
                "op": "append_to_section",
                "target_section": "## Important Requirements",
                "content": "- Inline close can touch JSON."
            }
        ],
        "changelog_entries": []
    });
    let payload = format!("```json {valid}```");

    let lowered = lower_trace2skill_json_patch(Trace2SkillPatchLoweringInput {
        parent: &parent,
        skill: &skill,
        payload: &payload,
        support_count: 2,
    })
    .unwrap();

    assert_eq!(lowered.plan.edits().len(), 1);
}

#[test]
fn trace2skill_json_patch_ignores_mid_line_fence_markers_before_real_fence() {
    let (parent, skill) = spreadsheet_skill_bank();
    let valid = json!({
        "reasoning": "real fence follows prose",
        "edits": [
            {
                "file": "SKILL.md",
                "op": "append_to_section",
                "target_section": "## Important Requirements",
                "content": "- Ignore prose fence markers."
            }
        ],
        "changelog_entries": []
    });
    let payload = format!("Respond in ```json format, then patch:\n```json\n{valid}\n```\n");

    let lowered = lower_trace2skill_json_patch(Trace2SkillPatchLoweringInput {
        parent: &parent,
        skill: &skill,
        payload: &payload,
        support_count: 2,
    })
    .unwrap();

    assert_eq!(lowered.plan.edits().len(), 1);
}

#[test]
fn trace2skill_json_patch_rejects_multiple_parseable_json_fences() {
    let (parent, skill) = spreadsheet_skill_bank();
    let payload = format!(
        "```json\n{}\n```\n```json\n{}\n```\n",
        json!({
            "reasoning": "first patch",
            "edits": [],
            "changelog_entries": []
        }),
        json!({
            "reasoning": "second patch",
            "edits": [],
            "changelog_entries": []
        })
    );

    let error = lower_trace2skill_json_patch(Trace2SkillPatchLoweringInput {
        parent: &parent,
        skill: &skill,
        payload: &payload,
        support_count: 2,
    })
    .unwrap_err();

    assert!(matches!(error, Trace2SkillPatchError::AmbiguousJsonFence));
}

#[test]
fn trace2skill_json_patch_accepts_longer_block_and_inline_fence_closers() {
    let (parent, skill) = spreadsheet_skill_bank();
    let block = json!({
        "reasoning": "longer close",
        "edits": [
            {
                "file": "SKILL.md",
                "op": "append_to_section",
                "target_section": "## Important Requirements",
                "content": "- Longer closing fences are valid Markdown."
            }
        ],
        "changelog_entries": []
    });
    let inline = json!({
        "reasoning": "inline longer close",
        "edits": [
            {
                "file": "SKILL.md",
                "op": "append_to_section",
                "target_section": "## Important Requirements",
                "content": "- Inline longer closing fences are valid too."
            }
        ],
        "changelog_entries": []
    });

    let lowered = lower_trace2skill_json_patch(Trace2SkillPatchLoweringInput {
        parent: &parent,
        skill: &skill,
        payload: &format!("````json\n{block}\n`````\n"),
        support_count: 2,
    })
    .unwrap();
    assert_eq!(lowered.plan.edits().len(), 1);

    let lowered = lower_trace2skill_json_patch(Trace2SkillPatchLoweringInput {
        parent: &parent,
        skill: &skill,
        payload: &format!("```json {inline}`````"),
        support_count: 2,
    })
    .unwrap();
    assert_eq!(lowered.plan.edits().len(), 1);
}

#[test]
fn trace2skill_json_patch_does_not_scan_inside_unclosed_non_json_fences() {
    let (parent, skill) = spreadsheet_skill_bank();
    let valid = json!({
        "reasoning": "nested text should not escape",
        "edits": [
            {
                "file": "SKILL.md",
                "op": "append_to_section",
                "target_section": "## Important Requirements",
                "content": "- This patch is text content, not an outer JSON fence."
            }
        ],
        "changelog_entries": []
    });
    let payload = format!("```text\nliteral prose\n```json\n{valid}\n```\n");

    let error = lower_trace2skill_json_patch(Trace2SkillPatchLoweringInput {
        parent: &parent,
        skill: &skill,
        payload: &payload,
        support_count: 2,
    })
    .unwrap_err();

    assert!(matches!(error, Trace2SkillPatchError::Json(_)));
}

#[test]
fn trace2skill_json_patch_accepts_single_parseable_patch_fence_after_invalid_patch_like_fence() {
    let (parent, skill) = spreadsheet_skill_bank();
    let valid = json!({
        "reasoning": "valid patch",
        "edits": [
            {
                "file": "SKILL.md",
                "op": "append_to_section",
                "target_section": "## Important Requirements",
                "content": "- Valid patch."
            }
        ],
        "changelog_entries": []
    });
    let payload = format!("```json\n{{\"edits\":\"not an array\"}}\n```\n```json\n{valid}\n```\n");

    let lowered = lower_trace2skill_json_patch(Trace2SkillPatchLoweringInput {
        parent: &parent,
        skill: &skill,
        payload: &payload,
        support_count: 2,
    })
    .unwrap();

    assert_eq!(lowered.plan.edits().len(), 1);
}

#[test]
fn trace2skill_json_patch_ignores_invalid_fence_openers() {
    let (parent, skill) = spreadsheet_skill_bank();
    let valid = json!({
        "reasoning": "valid patch",
        "edits": [
            {
                "file": "SKILL.md",
                "op": "append_to_section",
                "target_section": "## Important Requirements",
                "content": "- Valid patch."
            }
        ],
        "changelog_entries": []
    });
    let payload = format!(
        "    ```json\n{{\"edits\":[]}}\n```\n```json bad`info\n{{\"edits\":[]}}\n```\n```json\n{valid}\n```\n"
    );

    let lowered = lower_trace2skill_json_patch(Trace2SkillPatchLoweringInput {
        parent: &parent,
        skill: &skill,
        payload: &payload,
        support_count: 2,
    })
    .unwrap();

    assert_eq!(lowered.plan.edits().len(), 1);
}

#[test]
fn trace2skill_json_patch_rejects_under_scoped_and_ambiguous_text_edits() {
    let (parent, skill) = spreadsheet_skill_bank();
    let payload = fenced_json_patch(&json!({
        "reasoning": "Missing translated section anchor.",
        "edits": [
            {
                "file": "SKILL.md",
                "op": "replace_in_section",
                "old_text": "Preserve formulas",
                "content": "Preserve values"
            }
        ],
        "changelog_entries": []
    }));
    let error = lower_trace2skill_json_patch(Trace2SkillPatchLoweringInput {
        parent: &parent,
        skill: &skill,
        payload: &payload,
        support_count: 2,
    })
    .unwrap_err();
    assert!(matches!(
        error,
        Trace2SkillPatchError::MissingTargetSection { .. }
    ));

    let payload = fenced_json_patch(&json!({
        "reasoning": "Ambiguous replacement should fail.",
        "edits": [
            {
                "file": "SKILL.md",
                "op": "replace_in_section",
                "target_section": "## Important Requirements",
                "old_text": "e",
                "content": "plus"
            }
        ],
        "changelog_entries": []
    }));
    let error = lower_trace2skill_json_patch(Trace2SkillPatchLoweringInput {
        parent: &parent,
        skill: &skill,
        payload: &payload,
        support_count: 2,
    })
    .unwrap_err();
    assert!(matches!(error, Trace2SkillPatchError::AmbiguousText { .. }));

    let payload = fenced_json_patch(&json!({
        "reasoning": "Overlapping matches should still be ambiguous.",
        "edits": [
            {
                "file": "SKILL.md",
                "op": "replace_in_section",
                "target_section": "## Important Requirements",
                "old_text": "aa",
                "content": "e"
            }
        ],
        "changelog_entries": []
    }));
    let error = lower_trace2skill_json_patch(Trace2SkillPatchLoweringInput {
        parent: &parent,
        skill: &skill,
        payload: &payload,
        support_count: 2,
    })
    .unwrap_err();
    assert!(matches!(error, Trace2SkillPatchError::AmbiguousText { .. }));

    let payload = fenced_json_patch(&json!({
        "reasoning": "Replacement content should not be empty.",
        "edits": [
            {
                "file": "SKILL.md",
                "op": "replace_in_section",
                "target_section": "## Important Requirements",
                "old_text": "Preserve formulas",
                "content": "   "
            }
        ],
        "changelog_entries": []
    }));
    let error = lower_trace2skill_json_patch(Trace2SkillPatchLoweringInput {
        parent: &parent,
        skill: &skill,
        payload: &payload,
        support_count: 2,
    })
    .unwrap_err();
    assert!(matches!(
        error,
        Trace2SkillPatchError::EmptyPatchContent { .. }
    ));
}

#[test]
fn trace2skill_json_patch_requires_explicit_add_section_heading() {
    let (parent, skill) = spreadsheet_skill_bank();
    let payload = fenced_json_patch(&json!({
        "reasoning": "Missing target heading.",
        "edits": [
            {
                "file": "SKILL.md",
                "op": "add_section",
                "content": "- Do not invent a heading."
            }
        ],
        "changelog_entries": []
    }));

    let error = lower_trace2skill_json_patch(Trace2SkillPatchLoweringInput {
        parent: &parent,
        skill: &skill,
        payload: &payload,
        support_count: 2,
    })
    .unwrap_err();

    assert!(matches!(
        error,
        Trace2SkillPatchError::EmptyTargetSection { .. }
    ));
}

#[test]
fn trace2skill_json_patch_rejects_header_matches_and_duplicate_sections() {
    let (parent, skill) = spreadsheet_skill_bank();
    let payload = fenced_json_patch(&json!({
        "reasoning": "The section header should not be edited as body text.",
        "edits": [
            {
                "file": "SKILL.md",
                "op": "replace_in_section",
                "target_section": "## Important Requirements",
                "old_text": "## Important Requirements",
                "content": "## Other"
            }
        ],
        "changelog_entries": []
    }));
    let error = lower_trace2skill_json_patch(Trace2SkillPatchLoweringInput {
        parent: &parent,
        skill: &skill,
        payload: &payload,
        support_count: 2,
    })
    .unwrap_err();
    assert!(matches!(error, Trace2SkillPatchError::TextNotFound { .. }));

    let payload = fenced_json_patch(&json!({
        "reasoning": "Duplicate sections should be rejected.",
        "edits": [
            {
                "file": "SKILL.md",
                "op": "add_section",
                "target_section": "## Important Requirements",
                "content": "- duplicate"
            }
        ],
        "changelog_entries": []
    }));
    let error = lower_trace2skill_json_patch(Trace2SkillPatchLoweringInput {
        parent: &parent,
        skill: &skill,
        payload: &payload,
        support_count: 2,
    })
    .unwrap_err();
    assert!(matches!(
        error,
        Trace2SkillPatchError::DuplicateSection { .. }
    ));
}

#[test]
fn trace2skill_json_patch_handles_markdown_heading_edge_cases() {
    let skill = SkillName::new("xlsx").unwrap();
    let mut entries = BTreeMap::new();
    entries.insert(
        SkillPath::skill_md(),
        SkillFile::text(
            "---\nname: xlsx\ndescription: spreadsheet\n---\n# Skill\n\n## Important Requirements ##\n- one\n\n    ## Important Requirements\n    code sample\n\n## Later\n- two\n",
        ),
    );
    let parent =
        SkillBank::from_folders([SkillFolder::from_entries(skill.clone(), entries).unwrap()])
            .unwrap();
    let payload = fenced_json_patch(&json!({
        "reasoning": "Closing hashes are legal ATX heading syntax.",
        "edits": [
            {
                "file": "SKILL.md",
                "op": "append_to_section",
                "target_section": "## Important Requirements",
                "content": "- three"
            }
        ],
        "changelog_entries": []
    }));

    let lowered = lower_trace2skill_json_patch(Trace2SkillPatchLoweringInput {
        parent: &parent,
        skill: &skill,
        payload: &payload,
        support_count: 2,
    })
    .unwrap();

    assert_eq!(lowered.plan.edits().len(), 1);

    let payload = fenced_json_patch(&json!({
        "reasoning": "Whitespace-only anchors mean append at end.",
        "edits": [
            {
                "file": "SKILL.md",
                "op": "add_section",
                "target_section": "## Late Guidance",
                "after_section": "   ",
                "content": "- tail"
            }
        ],
        "changelog_entries": []
    }));

    let lowered = lower_trace2skill_json_patch(Trace2SkillPatchLoweringInput {
        parent: &parent,
        skill: &skill,
        payload: &payload,
        support_count: 2,
    })
    .unwrap();

    assert_eq!(lowered.plan.edits().len(), 1);
}

#[test]
fn trace2skill_json_patch_rejects_add_section_when_existing_heading_is_ambiguous() {
    let skill = SkillName::new("xlsx").unwrap();
    let mut entries = BTreeMap::new();
    entries.insert(
        SkillPath::skill_md(),
        SkillFile::text(
            "---\nname: xlsx\ndescription: spreadsheet\n---\n# Skill\n\n## Important Requirements\n- one\n\n## Important Requirements ##\n- two\n",
        ),
    );
    let parent =
        SkillBank::from_folders([SkillFolder::from_entries(skill.clone(), entries).unwrap()])
            .unwrap();
    let payload = fenced_json_patch(&json!({
        "reasoning": "Do not compound ambiguous section structure.",
        "edits": [
            {
                "file": "SKILL.md",
                "op": "add_section",
                "target_section": "## Important Requirements",
                "content": "- duplicate"
            }
        ],
        "changelog_entries": []
    }));

    let error = lower_trace2skill_json_patch(Trace2SkillPatchLoweringInput {
        parent: &parent,
        skill: &skill,
        payload: &payload,
        support_count: 2,
    })
    .unwrap_err();

    assert!(matches!(
        error,
        Trace2SkillPatchError::DuplicateSection { .. }
    ));
}

#[test]
fn trace2skill_json_patch_rejects_ambiguous_sections_and_ignores_fenced_headings() {
    let skill = SkillName::new("xlsx").unwrap();
    let mut entries = BTreeMap::new();
    entries.insert(
        SkillPath::skill_md(),
        SkillFile::text(
            "---\nname: xlsx\ndescription: spreadsheet\n---\n# Skill\n\n## Important Requirements\n- one\n\n```text\n## Important Requirements\n```\n\n## Later\n- two\n",
        ),
    );
    let parent =
        SkillBank::from_folders([SkillFolder::from_entries(skill.clone(), entries).unwrap()])
            .unwrap();
    let payload = fenced_json_patch(&json!({
        "reasoning": "Fenced heading text is not a section.",
        "edits": [
            {
                "file": "SKILL.md",
                "op": "append_to_section",
                "target_section": "## Important Requirements",
                "content": "- three"
            }
        ],
        "changelog_entries": []
    }));
    let lowered = lower_trace2skill_json_patch(Trace2SkillPatchLoweringInput {
        parent: &parent,
        skill: &skill,
        payload: &payload,
        support_count: 2,
    })
    .unwrap();
    assert_eq!(lowered.plan.edits().len(), 1);

    let mut entries = BTreeMap::new();
    entries.insert(
        SkillPath::skill_md(),
        SkillFile::text(
            "---\nname: xlsx\ndescription: spreadsheet\n---\n# Skill\n\n## Important Requirements\n- one\n\n## Important Requirements\n- two\n",
        ),
    );
    let parent =
        SkillBank::from_folders([SkillFolder::from_entries(skill.clone(), entries).unwrap()])
            .unwrap();
    let error = lower_trace2skill_json_patch(Trace2SkillPatchLoweringInput {
        parent: &parent,
        skill: &skill,
        payload: &payload,
        support_count: 2,
    })
    .unwrap_err();
    assert!(matches!(
        error,
        Trace2SkillPatchError::AmbiguousSection { .. }
    ));
}

#[test]
fn trace2skill_json_patch_keeps_info_string_openers_inside_active_fences() {
    let skill = SkillName::new("xlsx").unwrap();
    let mut entries = BTreeMap::new();
    entries.insert(
        SkillPath::skill_md(),
        SkillFile::text(
            "---\nname: xlsx\ndescription: spreadsheet\n---\n# Skill\n\n## Important Requirements\n- one\n\n```text\n```json\n## Not A Real Section\n```\n\n## Later\n- two\n",
        ),
    );
    let parent =
        SkillBank::from_folders([SkillFolder::from_entries(skill.clone(), entries).unwrap()])
            .unwrap();
    let payload = fenced_json_patch(&json!({
        "reasoning": "Nested-looking info strings are fence body text.",
        "edits": [
            {
                "file": "SKILL.md",
                "op": "append_to_section",
                "target_section": "## Not A Real Section",
                "content": "- should not find fenced heading"
            }
        ],
        "changelog_entries": []
    }));

    let error = lower_trace2skill_json_patch(Trace2SkillPatchLoweringInput {
        parent: &parent,
        skill: &skill,
        payload: &payload,
        support_count: 2,
    })
    .unwrap_err();

    assert!(matches!(
        error,
        Trace2SkillPatchError::SectionNotFound { .. }
    ));
}

#[test]
fn trace2skill_json_patch_rejects_empty_content_and_non_heading_add_section() {
    let (parent, skill) = spreadsheet_skill_bank();
    let payload = fenced_json_patch(&json!({
        "reasoning": "Empty appends are malformed.",
        "edits": [
            {
                "file": "SKILL.md",
                "op": "append_to_section",
                "target_section": "## Important Requirements"
            }
        ],
        "changelog_entries": []
    }));
    let error = lower_trace2skill_json_patch(Trace2SkillPatchLoweringInput {
        parent: &parent,
        skill: &skill,
        payload: &payload,
        support_count: 2,
    })
    .unwrap_err();
    assert!(matches!(
        error,
        Trace2SkillPatchError::EmptyPatchContent { .. }
    ));

    let payload = fenced_json_patch(&json!({
        "reasoning": "Plain text is not a section heading.",
        "edits": [
            {
                "file": "SKILL.md",
                "op": "add_section",
                "target_section": "Not a heading",
                "content": "- body"
            }
        ],
        "changelog_entries": []
    }));
    let error = lower_trace2skill_json_patch(Trace2SkillPatchLoweringInput {
        parent: &parent,
        skill: &skill,
        payload: &payload,
        support_count: 2,
    })
    .unwrap_err();
    assert!(matches!(
        error,
        Trace2SkillPatchError::InvalidSectionHeading { .. }
    ));

    let payload = fenced_json_patch(&json!({
        "reasoning": "Whitespace-only section targets are malformed.",
        "edits": [
            {
                "file": "SKILL.md",
                "op": "append_to_section",
                "target_section": "   ",
                "content": "- body"
            }
        ],
        "changelog_entries": []
    }));
    let error = lower_trace2skill_json_patch(Trace2SkillPatchLoweringInput {
        parent: &parent,
        skill: &skill,
        payload: &payload,
        support_count: 2,
    })
    .unwrap_err();
    assert!(matches!(
        error,
        Trace2SkillPatchError::MissingTargetSection { .. }
    ));

    let payload = fenced_json_patch(&json!({
        "reasoning": "Empty creates are malformed.",
        "edits": [
            {
                "file": "references/empty.md",
                "op": "create",
                "content": "   "
            }
        ],
        "changelog_entries": []
    }));
    let error = lower_trace2skill_json_patch(Trace2SkillPatchLoweringInput {
        parent: &parent,
        skill: &skill,
        payload: &payload,
        support_count: 2,
    })
    .unwrap_err();
    assert!(matches!(
        error,
        Trace2SkillPatchError::EmptyPatchContent { .. }
    ));
}

#[test]
fn trace2skill_json_patch_preserves_duplicate_inserted_reference_links() {
    let (parent, skill) = spreadsheet_skill_bank();
    let payload = fenced_json_patch(&json!({
        "reasoning": "Duplicate inserted link still counts as new inserted text.",
        "edits": [
            {
                "file": "SKILL.md",
                "op": "append_to_section",
                "target_section": "## Important Requirements",
                "content": "- Preserve formulas and verify outputs.\n- Read references/row-safety.md before deleting rows."
            },
            {
                "file": "references/row-safety.md",
                "op": "create",
                "content": "# Row Safety\n"
            }
        ],
        "changelog_entries": []
    }));

    let lowered = lower_trace2skill_json_patch(Trace2SkillPatchLoweringInput {
        parent: &parent,
        skill: &skill,
        payload: &payload,
        support_count: 2,
    })
    .unwrap();

    assert_eq!(lowered.plan.edits().len(), 2);
}

#[test]
fn trace2skill_json_patch_restricts_reference_creates_to_markdown() {
    let (parent, skill) = spreadsheet_skill_bank();
    let payload = fenced_json_patch(&json!({
        "reasoning": "Only markdown references are admissible.",
        "edits": [
            {
                "file": "references/script.py",
                "op": "create",
                "content": "print('no')"
            }
        ],
        "changelog_entries": []
    }));

    let error = lower_trace2skill_json_patch(Trace2SkillPatchLoweringInput {
        parent: &parent,
        skill: &skill,
        payload: &payload,
        support_count: 2,
    })
    .unwrap_err();

    assert!(matches!(
        error,
        Trace2SkillPatchError::UnsupportedPatchPath { .. }
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
             - Preserve formulas and verify outputs.\n\
             - aaa\n",
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
