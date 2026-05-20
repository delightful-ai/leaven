//! `Trace2Skill` JSON patch lowering into Leaven skill primitives.

use std::{collections::BTreeMap, str};

use leaven_agentic_skill::{
    SkillParsedPatchDocument, SkillParsedPatchError, SkillParsedPatchOperation,
    SkillPatchApplication, SkillPatchApplicationError, SkillPatchFileRef, SkillPatchPlan,
    SkillPatchRange, SkillPatchSupport,
};
use leaven_artifact_skill::{SkillBank, SkillBankChange, SkillFile, SkillName, SkillPath};
use serde::Deserialize;

/// Inputs for lowering one upstream `Trace2Skill` JSON patch.
#[derive(Clone, Copy, Debug)]
pub struct Trace2SkillPatchLoweringInput<'a> {
    /// Parent skill bank the upstream patch was authored against.
    pub parent: &'a SkillBank,
    /// Skill folder targeted by the upstream runner.
    pub skill: &'a SkillName,
    /// Upstream JSON patch object or an LLM response containing a fenced JSON patch.
    pub payload: &'a str,
    /// Number of independent analyst patches supporting this patch.
    pub support_count: u32,
}

/// Lowered patch ready for Leaven's validated skill patch application path.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Trace2SkillPatchLowering {
    /// Upstream patch reasoning text.
    pub reasoning: String,
    /// Upstream changelog entries.
    pub changelog_entries: Vec<String>,
    /// Validated Leaven patch plan derived from changed skill files.
    pub plan: SkillPatchPlan,
    /// Concrete artifact-native changes to apply atomically.
    pub changes: Vec<SkillBankChange>,
}

/// Lowers one upstream `Trace2Skill` JSON patch into Leaven skill primitives.
///
/// This bridge intentionally accepts the upstream `PATCH_FORMAT=json` edit
/// schema from `skill_evolver/parallel_evolving_agent.py`. It does not own
/// `Trace2Skill` scheduling, prompts, merge policy, or prevalence thresholds.
pub fn lower_trace2skill_json_patch(
    input: Trace2SkillPatchLoweringInput<'_>,
) -> Result<Trace2SkillPatchLowering, Trace2SkillPatchError> {
    let payload = extract_json_payload(input.payload)?;
    let patch: UpstreamJsonPatch = serde_json::from_str(payload)?;
    let support = SkillPatchSupport::new(input.support_count)
        .map_err(|error| Trace2SkillPatchError::ParsedPatch(SkillParsedPatchError::Plan(error)))?;
    let folder =
        input
            .parent
            .get(input.skill)
            .ok_or_else(|| Trace2SkillPatchError::MissingSkill {
                skill: input.skill.to_string(),
            })?;
    let original = folder.entries();
    let mut updated = original.clone();

    for edit in &patch.edits {
        apply_upstream_edit(&mut updated, edit)?;
    }

    let mut operations = Vec::new();
    for (path, before, after) in changed_paths(original, &updated) {
        let target = SkillPatchFileRef::new(input.skill.clone(), path.clone());
        match (before, after) {
            (None, Some(file)) => {
                operations.push(SkillParsedPatchOperation::create_file(
                    target,
                    support,
                    file.clone(),
                ));
            }
            (Some(_), None) => {
                if path.is_skill_md() {
                    return Err(Trace2SkillPatchError::CannotDeleteSkillMd);
                }
                operations.push(SkillParsedPatchOperation::delete_file(target, support));
            }
            (Some(_), Some(file)) => {
                let operation = SkillParsedPatchOperation::modify_file(
                    target,
                    SkillPatchRange::WholeFile,
                    support,
                    file.clone(),
                );
                let operation = if path.is_skill_md() {
                    operation.with_reference_links_from_text(skill_file_text(file, &path)?)
                } else {
                    operation
                };
                operations.push(operation);
            }
            (None, None) => {}
        }
    }
    let parsed = SkillParsedPatchDocument::new(operations).validate_against(input.parent)?;
    let (plan, changes) = parsed.into_parts();

    Ok(Trace2SkillPatchLowering {
        reasoning: patch.reasoning,
        changelog_entries: patch.changelog_entries,
        plan,
        changes,
    })
}

/// Lowers and atomically applies one upstream `Trace2Skill` JSON patch.
pub fn apply_trace2skill_json_patch(
    input: Trace2SkillPatchLoweringInput<'_>,
) -> Result<SkillPatchApplication, Trace2SkillPatchError> {
    let parent = input.parent;
    let lowered = lower_trace2skill_json_patch(input)?;
    Ok(SkillPatchApplication::apply(
        parent,
        lowered.plan,
        lowered.changes,
    )?)
}

fn extract_json_payload(payload: &str) -> Result<&str, Trace2SkillPatchError> {
    let trimmed = payload.trim();
    let Some(marker_start) = trimmed.find("```json") else {
        return Ok(trimmed);
    };
    let after_marker = &trimmed[marker_start..];
    let Some(first_newline) = after_marker.find('\n') else {
        return Err(Trace2SkillPatchError::UnclosedJsonFence);
    };
    let body = &after_marker[first_newline + 1..];
    let Some(end) = body.find("```") else {
        return Err(Trace2SkillPatchError::UnclosedJsonFence);
    };
    Ok(body[..end].trim())
}

fn changed_paths<'a>(
    original: &'a BTreeMap<SkillPath, SkillFile>,
    updated: &'a BTreeMap<SkillPath, SkillFile>,
) -> Vec<(SkillPath, Option<&'a SkillFile>, Option<&'a SkillFile>)> {
    let mut paths = original.keys().chain(updated.keys()).collect::<Vec<_>>();
    paths.sort();
    paths.dedup();
    paths
        .into_iter()
        .filter_map(|path| {
            let before = original.get(path);
            let after = updated.get(path);
            if before == after {
                None
            } else {
                Some((path.clone(), before, after))
            }
        })
        .collect()
}

fn apply_upstream_edit(
    state: &mut BTreeMap<SkillPath, SkillFile>,
    edit: &UpstreamJsonEdit,
) -> Result<(), Trace2SkillPatchError> {
    let path = skill_patch_path(&edit.file)?;
    match edit.op()? {
        UpstreamPatchOp::Create => create_file(state, path, &edit.content),
        UpstreamPatchOp::DeleteFile => delete_file(state, &path),
        UpstreamPatchOp::AppendToSection => append_to_section(state, &path, edit),
        UpstreamPatchOp::ReplaceInSection => replace_in_section(state, &path, edit),
        UpstreamPatchOp::InsertAfter | UpstreamPatchOp::InsertBefore => {
            insert_relative_to_text(state, &path, edit)
        }
        UpstreamPatchOp::AddSection => add_section(state, &path, edit),
        UpstreamPatchOp::DeleteSection => delete_section(state, &path, edit),
    }
}

fn create_file(
    state: &mut BTreeMap<SkillPath, SkillFile>,
    path: SkillPath,
    content: &str,
) -> Result<(), Trace2SkillPatchError> {
    if path.is_skill_md() {
        return Err(Trace2SkillPatchError::CannotCreateSkillMd);
    }
    if state.contains_key(&path) {
        return Err(Trace2SkillPatchError::CreateOverwritesExisting {
            path: path.to_string(),
        });
    }
    state.insert(path, SkillFile::text(content.to_owned()));
    Ok(())
}

fn delete_file(
    state: &mut BTreeMap<SkillPath, SkillFile>,
    path: &SkillPath,
) -> Result<(), Trace2SkillPatchError> {
    if path.is_skill_md() {
        return Err(Trace2SkillPatchError::CannotDeleteSkillMd);
    }
    state
        .remove(path)
        .map(|_| ())
        .ok_or_else(|| Trace2SkillPatchError::MissingPatchFile {
            path: path.to_string(),
        })
}

fn append_to_section(
    state: &mut BTreeMap<SkillPath, SkillFile>,
    path: &SkillPath,
    edit: &UpstreamJsonEdit,
) -> Result<(), Trace2SkillPatchError> {
    update_text_file(state, path, |content| {
        let mut lines = split_lines(content);
        let (_, end) = section_bounds(&lines, &edit.target_section, path)?;
        let mut insert_at = end;
        while insert_at > 0 && lines[insert_at - 1].trim().is_empty() {
            insert_at -= 1;
        }
        lines.splice(insert_at..insert_at, prefixed_block(&edit.content));
        Ok(lines.join("\n"))
    })
}

fn replace_in_section(
    state: &mut BTreeMap<SkillPath, SkillFile>,
    path: &SkillPath,
    edit: &UpstreamJsonEdit,
) -> Result<(), Trace2SkillPatchError> {
    update_text_file(state, path, |content| {
        if edit.old_text.is_empty() {
            return Err(Trace2SkillPatchError::EmptyOldText {
                path: path.to_string(),
            });
        }
        if edit.target_section.is_empty() {
            return replace_text(content, &edit.old_text, &edit.content, path);
        }
        let mut lines = split_lines(content);
        let (start, end) = section_bounds(&lines, &edit.target_section, path)?;
        let section = lines[start..end].join("\n");
        let replacement = replace_text(&section, &edit.old_text, &edit.content, path)?;
        lines.splice(start..end, split_lines(&replacement));
        Ok(lines.join("\n"))
    })
}

fn insert_relative_to_text(
    state: &mut BTreeMap<SkillPath, SkillFile>,
    path: &SkillPath,
    edit: &UpstreamJsonEdit,
) -> Result<(), Trace2SkillPatchError> {
    update_text_file(state, path, |content| {
        if edit.target_text.is_empty() {
            return Err(Trace2SkillPatchError::EmptyTargetText {
                path: path.to_string(),
            });
        }
        let mut lines = split_lines(content);
        let (offset, span) = if edit.target_section.is_empty() {
            (0, lines.len())
        } else {
            let (start, end) = section_bounds(&lines, &edit.target_section, path)?;
            (start, end - start)
        };
        let section = lines[offset..offset + span].join("\n");
        let replacement = match edit.op()? {
            UpstreamPatchOp::InsertAfter => replace_text(
                &section,
                &edit.target_text,
                &format!("{}\n{}", edit.target_text, edit.content),
                path,
            )?,
            UpstreamPatchOp::InsertBefore => replace_text(
                &section,
                &edit.target_text,
                &format!("{}\n{}", edit.content, edit.target_text),
                path,
            )?,
            _ => unreachable!(),
        };
        lines.splice(offset..offset + span, split_lines(&replacement));
        Ok(lines.join("\n"))
    })
}

fn add_section(
    state: &mut BTreeMap<SkillPath, SkillFile>,
    path: &SkillPath,
    edit: &UpstreamJsonEdit,
) -> Result<(), Trace2SkillPatchError> {
    update_text_file(state, path, |content| {
        let mut lines = split_lines(content);
        let insert_at = if edit.after_section.is_empty() {
            lines.len()
        } else {
            section_bounds(&lines, &edit.after_section, path)
                .map(|(_, end)| end)
                .unwrap_or(lines.len())
        };
        let header = if edit.target_section.is_empty() {
            "## New Section"
        } else {
            &edit.target_section
        };
        let mut block = vec![String::new(), header.to_owned(), String::new()];
        block.extend(split_lines(&edit.content));
        lines.splice(insert_at..insert_at, block);
        Ok(lines.join("\n"))
    })
}

fn delete_section(
    state: &mut BTreeMap<SkillPath, SkillFile>,
    path: &SkillPath,
    edit: &UpstreamJsonEdit,
) -> Result<(), Trace2SkillPatchError> {
    update_text_file(state, path, |content| {
        let mut lines = split_lines(content);
        let (mut start, end) = section_bounds(&lines, &edit.target_section, path)?;
        while start > 0 && lines[start - 1].trim().is_empty() {
            start -= 1;
        }
        lines.splice(start..end, Vec::<String>::new());
        Ok(lines.join("\n"))
    })
}

fn skill_patch_path(raw: &str) -> Result<SkillPath, Trace2SkillPatchError> {
    if raw.is_empty() {
        return Err(Trace2SkillPatchError::EmptyPatchFile);
    }
    let path = SkillPath::new(raw).map_err(|source| Trace2SkillPatchError::InvalidPatchPath {
        path: raw.to_owned(),
        source,
    })?;
    if path.as_str() == "LICENSE.txt" || path.as_str() == "recalc.py" {
        return Err(Trace2SkillPatchError::ProtectedPatchFile {
            path: path.to_string(),
        });
    }
    if !path.is_skill_md() && !path.as_str().starts_with("references/") {
        return Err(Trace2SkillPatchError::UnsupportedPatchPath {
            path: path.to_string(),
        });
    }
    Ok(path)
}

fn update_text_file(
    state: &mut BTreeMap<SkillPath, SkillFile>,
    path: &SkillPath,
    update: impl FnOnce(&str) -> Result<String, Trace2SkillPatchError>,
) -> Result<(), Trace2SkillPatchError> {
    let file = state
        .get(path)
        .ok_or_else(|| Trace2SkillPatchError::MissingPatchFile {
            path: path.to_string(),
        })?;
    let permissions = file.permissions();
    let text = skill_file_text(file, path)?;
    let next = update(text)?;
    state.insert(
        path.clone(),
        SkillFile::with_permissions(next.into_bytes(), permissions),
    );
    Ok(())
}

fn section_bounds(
    lines: &[String],
    section_header: &str,
    path: &SkillPath,
) -> Result<(usize, usize), Trace2SkillPatchError> {
    let target = section_header.trim();
    let level = heading_level(target);
    if level == 0 {
        return Err(Trace2SkillPatchError::SectionNotFound {
            path: path.to_string(),
            section: target.to_owned(),
        });
    }
    for (index, line) in lines.iter().enumerate() {
        if line.trim() != target {
            continue;
        }
        let end = lines[index + 1..]
            .iter()
            .position(|candidate| {
                let candidate_level = heading_level(candidate);
                candidate_level > 0 && candidate_level <= level
            })
            .map_or(lines.len(), |relative| index + 1 + relative);
        return Ok((index, end));
    }
    Err(Trace2SkillPatchError::SectionNotFound {
        path: path.to_string(),
        section: target.to_owned(),
    })
}

fn heading_level(line: &str) -> usize {
    line.chars()
        .take_while(|character| *character == '#')
        .count()
}

fn replace_text(
    content: &str,
    old_text: &str,
    replacement: &str,
    path: &SkillPath,
) -> Result<String, Trace2SkillPatchError> {
    if !content.contains(old_text) {
        return Err(Trace2SkillPatchError::TextNotFound {
            path: path.to_string(),
            text: old_text.to_owned(),
        });
    }
    Ok(content.replacen(old_text, replacement, 1))
}

fn split_lines(content: &str) -> Vec<String> {
    content.split('\n').map(str::to_owned).collect()
}

fn prefixed_block(content: &str) -> Vec<String> {
    let mut block = vec![String::new()];
    block.extend(split_lines(content));
    block
}

fn skill_file_text<'a>(
    file: &'a SkillFile,
    path: &SkillPath,
) -> Result<&'a str, Trace2SkillPatchError> {
    str::from_utf8(file.bytes()).map_err(|_| Trace2SkillPatchError::NonUtf8PatchFile {
            path: path.to_string(),
        })
}

#[derive(Debug, Deserialize)]
struct UpstreamJsonPatch {
    #[serde(default)]
    reasoning: String,
    #[serde(default)]
    edits: Vec<UpstreamJsonEdit>,
    #[serde(default)]
    changelog_entries: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct UpstreamJsonEdit {
    file: String,
    op: String,
    #[serde(default)]
    target_section: String,
    #[serde(default)]
    target_text: String,
    #[serde(default)]
    content: String,
    #[serde(default)]
    old_text: String,
    #[serde(default)]
    after_section: String,
}

impl UpstreamJsonEdit {
    fn op(&self) -> Result<UpstreamPatchOp, Trace2SkillPatchError> {
        match self.op.as_str() {
            "insert_after" => Ok(UpstreamPatchOp::InsertAfter),
            "insert_before" => Ok(UpstreamPatchOp::InsertBefore),
            "append_to_section" => Ok(UpstreamPatchOp::AppendToSection),
            "replace_in_section" => Ok(UpstreamPatchOp::ReplaceInSection),
            "add_section" => Ok(UpstreamPatchOp::AddSection),
            "delete_section" => Ok(UpstreamPatchOp::DeleteSection),
            "create" | "create_file" | "createFile" => Ok(UpstreamPatchOp::Create),
            "delete_file" | "deleteFile" => Ok(UpstreamPatchOp::DeleteFile),
            op => Err(Trace2SkillPatchError::UnsupportedPatchOp { op: op.to_owned() }),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum UpstreamPatchOp {
    InsertAfter,
    InsertBefore,
    AppendToSection,
    ReplaceInSection,
    AddSection,
    DeleteSection,
    Create,
    DeleteFile,
}

/// Error while lowering an upstream `Trace2Skill` patch.
#[derive(Debug, thiserror::Error)]
pub enum Trace2SkillPatchError {
    /// Fenced JSON block was opened but not closed.
    #[error("Trace2Skill patch contains an unclosed fenced json block")]
    UnclosedJsonFence,
    /// JSON parsing failed.
    #[error("failed to parse Trace2Skill patch JSON: {0}")]
    Json(#[from] serde_json::Error),
    /// The targeted skill folder is absent from the parent bank.
    #[error("Trace2Skill patch targets missing skill {skill}")]
    MissingSkill {
        /// Missing skill name.
        skill: String,
    },
    /// File path was empty.
    #[error("Trace2Skill patch edit is missing its file path")]
    EmptyPatchFile,
    /// File path was not a valid skill-relative path.
    #[error("invalid Trace2Skill patch path {path}: {source}")]
    InvalidPatchPath {
        /// Invalid path.
        path: String,
        /// Path validation error.
        source: leaven_artifact_skill::SkillPathError,
    },
    /// Upstream tried to edit a file Leaven does not allow this bridge to edit.
    #[error("Trace2Skill patch path is outside SKILL.md/references: {path}")]
    UnsupportedPatchPath {
        /// Unsupported path.
        path: String,
    },
    /// Upstream tried to mutate a protected support file.
    #[error("Trace2Skill patch targets protected file {path}")]
    ProtectedPatchFile {
        /// Protected path.
        path: String,
    },
    /// Operation is not part of the upstream JSON patch schema.
    #[error("unsupported Trace2Skill patch op {op}")]
    UnsupportedPatchOp {
        /// Unsupported operation.
        op: String,
    },
    /// Patch targets a file absent from the parent skill.
    #[error("Trace2Skill patch targets missing file {path}")]
    MissingPatchFile {
        /// Missing path.
        path: String,
    },
    /// Patch attempts to overwrite an existing file with a create op.
    #[error("Trace2Skill create op would overwrite existing file {path}")]
    CreateOverwritesExisting {
        /// Existing path.
        path: String,
    },
    /// The required root file cannot be created by a file create operation.
    #[error("Trace2Skill patch cannot create SKILL.md")]
    CannotCreateSkillMd,
    /// The required root file cannot be deleted.
    #[error("Trace2Skill patch cannot delete SKILL.md")]
    CannotDeleteSkillMd,
    /// Existing file bytes are not UTF-8 and cannot be patched as markdown.
    #[error("Trace2Skill patch target {path} is not UTF-8")]
    NonUtf8PatchFile {
        /// Non-UTF-8 path.
        path: String,
    },
    /// A section-based operation could not find its heading.
    #[error("Trace2Skill patch could not find section {section:?} in {path}")]
    SectionNotFound {
        /// Target path.
        path: String,
        /// Missing heading.
        section: String,
    },
    /// Insert operation lacks a target string.
    #[error("Trace2Skill patch insert op for {path} has empty target_text")]
    EmptyTargetText {
        /// Target path.
        path: String,
    },
    /// Replacement operation lacks old text.
    #[error("Trace2Skill patch replace op for {path} has empty old_text")]
    EmptyOldText {
        /// Target path.
        path: String,
    },
    /// Exact target text was not found after translation.
    #[error("Trace2Skill patch could not find text {text:?} in {path}")]
    TextNotFound {
        /// Target path.
        path: String,
        /// Missing text.
        text: String,
    },
    /// Parsed patch lowering or plan validation failed.
    #[error(transparent)]
    ParsedPatch(#[from] SkillParsedPatchError),
    /// Atomic skill application failed.
    #[error(transparent)]
    Application(#[from] SkillPatchApplicationError),
}
