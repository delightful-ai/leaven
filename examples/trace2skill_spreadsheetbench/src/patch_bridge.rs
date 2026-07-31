//! `Trace2Skill` JSON patch lowering into Leaven skill primitives.

use std::{
    collections::{BTreeMap, BTreeSet},
    str,
};

use leaven_agentic_skill::{
    SkillParsedPatchDocument, SkillParsedPatchError, SkillParsedPatchOperation,
    SkillPatchApplication, SkillPatchApplicationError, SkillPatchFileRef, SkillPatchPlan,
    SkillPatchRange, SkillPatchSupport, SkillReferencePath,
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
    let patch: UpstreamJsonPatch = match serde_json::from_str(input.payload.trim()) {
        Ok(patch) => patch,
        Err(raw_error) => parse_fenced_json_patch(input.payload, raw_error)?,
    };
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

    let skill_md_after = updated
        .get(&SkillPath::skill_md())
        .map(|file| skill_file_text(file, &SkillPath::skill_md()))
        .transpose()?;
    let changed = changed_paths(original, &updated);
    let touched_reference_paths = changed
        .iter()
        .filter_map(|(path, before, after)| {
            (before != after)
                .then(|| SkillReferencePath::new(path.clone()).ok())
                .flatten()
        })
        .collect::<BTreeSet<_>>();
    let mut operations = Vec::new();
    let mut saw_skill_md_operation = false;
    let mut saw_reference_file_operation = false;
    for (path, before, after) in changed {
        let target = SkillPatchFileRef::new(input.skill.clone(), path.clone());
        match (before, after) {
            (None, Some(file)) => {
                if path.as_str().starts_with("references/") {
                    saw_reference_file_operation = true;
                }
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
                if path.as_str().starts_with("references/") {
                    saw_reference_file_operation = true;
                }
                operations.push(SkillParsedPatchOperation::delete_file(target, support));
            }
            (Some(before_file), Some(file)) => {
                let operation = SkillParsedPatchOperation::modify_file(
                    target,
                    SkillPatchRange::WholeFile,
                    support,
                    file.clone(),
                );
                let operation = if path.is_skill_md() {
                    saw_skill_md_operation = true;
                    operation.with_reference_links(skill_md_reference_links_for_edit(
                        skill_file_text(before_file, &path)?,
                        skill_file_text(file, &path)?,
                        &touched_reference_paths,
                    ))
                } else {
                    operation
                };
                operations.push(operation);
            }
            (None, None) => {}
        }
    }
    if saw_reference_file_operation
        && !saw_skill_md_operation
        && let (Some(skill_md_before), Some(skill_md_after)) =
            (original.get(&SkillPath::skill_md()), skill_md_after)
    {
        operations.push(
            SkillParsedPatchOperation::modify_file(
                SkillPatchFileRef::new(input.skill.clone(), SkillPath::skill_md()),
                SkillPatchRange::WholeFile,
                support,
                SkillFile::with_permissions(
                    skill_md_after.as_bytes().to_vec(),
                    skill_md_before.permissions(),
                ),
            )
            .with_reference_links(skill_md_reference_links_for_edit(
                skill_file_text(skill_md_before, &SkillPath::skill_md())?,
                &skill_md_after,
                &touched_reference_paths,
            )),
        );
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

fn parse_fenced_json_patch(
    payload: &str,
    raw_error: serde_json::Error,
) -> Result<UpstreamJsonPatch, Trace2SkillPatchError> {
    let candidates = extract_json_payloads(payload)?;
    if candidates.is_empty() {
        return Err(Trace2SkillPatchError::Json(raw_error));
    }
    let mut parsed = None;
    let mut last_error = raw_error;
    for candidate in candidates {
        if !is_patch_like_json(candidate) {
            continue;
        }
        match serde_json::from_str::<UpstreamJsonPatch>(candidate) {
            Ok(patch) => {
                if parsed.replace(patch).is_some() {
                    return Err(Trace2SkillPatchError::AmbiguousJsonFence);
                }
            }
            Err(error) => last_error = error,
        }
    }
    parsed.ok_or(Trace2SkillPatchError::Json(last_error))
}

fn is_patch_like_json(candidate: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(candidate)
        .ok()
        .is_some_and(|value| {
            value
                .as_object()
                .is_some_and(|object| object.contains_key("edits"))
        })
}

fn extract_json_payloads(payload: &str) -> Result<Vec<&str>, Trace2SkillPatchError> {
    let mut candidates = Vec::new();
    let mut search_start = 0;
    while let Some((open, delimiter_len)) = find_next_fence(payload, search_start) {
        let delimiter = &payload[open..open + delimiter_len];
        let tag_start = open + delimiter_len;
        let line_end = payload[tag_start..]
            .find('\n')
            .map_or(payload.len(), |relative| tag_start + relative);
        let tag_line = &payload[tag_start..line_end];
        let leading = tag_line.len() - tag_line.trim_start().len();
        let trimmed_tag_line = tag_line.trim_start();
        let Some(after_tag) = strip_json_fence_tag(trimmed_tag_line) else {
            let body_start = if line_end < payload.len() {
                line_end + 1
            } else {
                line_end
            };
            search_start = find_closing_fence(payload, body_start, delimiter)
                .map_or(payload.len(), |(_, after_close)| after_close);
            continue;
        };
        let after_tag_start = tag_start + leading + trimmed_tag_line.len() - after_tag.len();
        let inline_body = after_tag.trim_start();
        let inline_body_start = after_tag_start + after_tag.len() - inline_body.len();
        if let Some(relative_close) = inline_closing_fence(inline_body, delimiter) {
            candidates.push(inline_body[..relative_close].trim());
            search_start = inline_body_start + relative_close + delimiter_len;
            continue;
        }
        let body_start = if line_end < payload.len() {
            line_end + 1
        } else {
            line_end
        };
        let Some((body_end, after_close)) = find_closing_fence(payload, body_start, delimiter)
        else {
            return Err(Trace2SkillPatchError::UnclosedJsonFence);
        };
        candidates.push(payload[body_start..body_end].trim());
        search_start = after_close;
    }
    Ok(candidates)
}

fn find_next_fence(payload: &str, start: usize) -> Option<(usize, usize)> {
    let mut offset = start;
    for line in payload[start..].split_inclusive('\n') {
        let at_line_start = offset == 0 || payload.as_bytes().get(offset - 1) == Some(&b'\n');
        if at_line_start {
            if let Some((leading, _, delimiter_len)) =
                markdown_fence_opener(line).or_else(|| inline_json_fence_opener(line))
            {
                return Some((offset + leading, delimiter_len));
            }
        }
        offset += line.len();
    }
    None
}

fn inline_json_fence_opener(line: &str) -> Option<(usize, u8, usize)> {
    let leading = leading_markdown_spaces(line);
    if leading > 3 {
        return None;
    }
    let trimmed = &line[leading..];
    let marker = *trimmed.as_bytes().first()?;
    if marker != b'`' && marker != b'~' {
        return None;
    }
    let len = trimmed.bytes().take_while(|byte| *byte == marker).count();
    if len < 3 {
        return None;
    }
    let delimiter = &trimmed[..len];
    let after_tag = strip_json_fence_tag(&trimmed[len..])?;
    inline_closing_fence(after_tag.trim_start(), delimiter)?;
    Some((leading, marker, len))
}

fn inline_closing_fence(line: &str, delimiter: &str) -> Option<usize> {
    let trimmed = line.trim_end();
    let marker = *delimiter.as_bytes().first()?;
    let closing_len = trimmed
        .bytes()
        .rev()
        .take_while(|byte| *byte == marker)
        .count();
    if closing_len < delimiter.len() {
        return None;
    }
    Some(trimmed.len() - closing_len)
}

fn strip_json_fence_tag(line: &str) -> Option<&str> {
    let tag = line.get(..4)?;
    if !tag.eq_ignore_ascii_case("json") {
        return None;
    }
    let rest = &line[4..];
    if rest.is_empty() || rest.starts_with(char::is_whitespace) {
        return Some(rest);
    }
    None
}

fn find_closing_fence(payload: &str, start: usize, delimiter: &str) -> Option<(usize, usize)> {
    let mut offset = start;
    let marker = *delimiter.as_bytes().first()?;
    let opener_len = delimiter.len();
    for line in payload[start..].split_inclusive('\n') {
        if markdown_fence_closer(line, marker, opener_len).is_some() {
            let body_end = offset;
            let after_close = offset + line.len();
            return Some((body_end, after_close));
        }
        offset += line.len();
    }
    None
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
    if content.trim().is_empty() {
        return Err(Trace2SkillPatchError::EmptyPatchContent {
            path: path.to_string(),
            op: "create".to_owned(),
        });
    }
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
        require_non_empty_content(edit, path)?;
        if edit.target_section.trim().is_empty() {
            return Err(Trace2SkillPatchError::MissingTargetSection {
                path: path.to_string(),
                op: edit.op.clone(),
            });
        }
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
        require_non_empty_content(edit, path)?;
        if edit.old_text.is_empty() {
            return Err(Trace2SkillPatchError::EmptyOldText {
                path: path.to_string(),
            });
        }
        if edit.target_section.trim().is_empty() {
            return Err(Trace2SkillPatchError::MissingTargetSection {
                path: path.to_string(),
                op: edit.op.clone(),
            });
        }
        let mut lines = split_lines(content);
        let (start, end) = section_bounds(&lines, &edit.target_section, path)?;
        let body_start = start + 1;
        let section = lines[body_start..end].join("\n");
        let replacement = replace_text(&section, &edit.old_text, &edit.content, path)?;
        lines.splice(body_start..end, split_lines(&replacement));
        Ok(lines.join("\n"))
    })
}

fn insert_relative_to_text(
    state: &mut BTreeMap<SkillPath, SkillFile>,
    path: &SkillPath,
    edit: &UpstreamJsonEdit,
) -> Result<(), Trace2SkillPatchError> {
    update_text_file(state, path, |content| {
        require_non_empty_content(edit, path)?;
        if edit.target_text.is_empty() {
            return Err(Trace2SkillPatchError::EmptyTargetText {
                path: path.to_string(),
            });
        }
        if edit.target_section.trim().is_empty() {
            return Err(Trace2SkillPatchError::MissingTargetSection {
                path: path.to_string(),
                op: edit.op.clone(),
            });
        }
        let mut lines = split_lines(content);
        let (start, end) = section_bounds(&lines, &edit.target_section, path)?;
        let offset = start + 1;
        let span = end - offset;
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
        require_non_empty_content(edit, path)?;
        let mut lines = split_lines(content);
        if edit.target_section.trim().is_empty() {
            return Err(Trace2SkillPatchError::EmptyTargetSection {
                path: path.to_string(),
                op: edit.op.clone(),
            });
        }
        if heading_level(&edit.target_section) == 0 {
            return Err(Trace2SkillPatchError::InvalidSectionHeading {
                path: path.to_string(),
                section: edit.target_section.clone(),
            });
        }
        match section_bounds(&lines, &edit.target_section, path) {
            Ok(_) | Err(Trace2SkillPatchError::AmbiguousSection { .. }) => {
                return Err(Trace2SkillPatchError::DuplicateSection {
                    path: path.to_string(),
                    section: edit.target_section.trim().to_owned(),
                });
            }
            Err(Trace2SkillPatchError::SectionNotFound { .. }) => {}
            Err(error) => return Err(error),
        }
        let after_section = edit.after_section.trim();
        let insert_at = if after_section.is_empty() {
            lines.len()
        } else {
            section_bounds(&lines, after_section, path).map(|(_, end)| end)?
        };
        let mut block = vec![String::new(), edit.target_section.clone(), String::new()];
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
        if edit.target_section.trim().is_empty() {
            return Err(Trace2SkillPatchError::MissingTargetSection {
                path: path.to_string(),
                op: edit.op.clone(),
            });
        }
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
    if path.as_str().starts_with("references/") && !path.as_str().ends_with(".md") {
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
    let Some((level, target_text)) = heading_key(target) else {
        return Err(Trace2SkillPatchError::SectionNotFound {
            path: path.to_string(),
            section: target.to_owned(),
        });
    };
    let mut found = None;
    let mut active_fence = None;
    for (index, line) in lines.iter().enumerate() {
        if update_markdown_fence(&mut active_fence, line) {
            continue;
        }
        if active_fence.is_some() {
            continue;
        }
        if heading_key(line).as_ref() != Some(&(level, target_text.clone())) {
            continue;
        }
        let end = lines[index + 1..]
            .iter()
            .scan(None, |active_fence, candidate| {
                if update_markdown_fence(active_fence, candidate) {
                    return Some(false);
                }
                if active_fence.is_some() {
                    return Some(false);
                }
                let candidate_level = heading_level(candidate);
                Some(candidate_level > 0 && candidate_level <= level)
            })
            .position(|is_boundary| is_boundary)
            .map_or(lines.len(), |relative| index + 1 + relative);
        if found.replace((index, end)).is_some() {
            return Err(Trace2SkillPatchError::AmbiguousSection {
                path: path.to_string(),
                section: target.to_owned(),
            });
        }
    }
    found.ok_or_else(|| Trace2SkillPatchError::SectionNotFound {
        path: path.to_string(),
        section: target.to_owned(),
    })
}

fn update_markdown_fence(active: &mut Option<(u8, usize)>, line: &str) -> bool {
    match *active {
        Some((active_marker, active_len)) => {
            if markdown_fence_closer(line, active_marker, active_len).is_some() {
                *active = None;
                true
            } else {
                false
            }
        }
        None => {
            let Some((_, marker, len)) = markdown_fence_opener(line) else {
                return false;
            };
            *active = Some((marker, len));
            true
        }
    }
}

fn markdown_fence_opener(line: &str) -> Option<(usize, u8, usize)> {
    let leading = leading_markdown_spaces(line);
    if leading > 3 {
        return None;
    }
    let trimmed = &line[leading..];
    let marker = *trimmed.as_bytes().first()?;
    if marker != b'`' && marker != b'~' {
        return None;
    }
    let len = trimmed.bytes().take_while(|byte| *byte == marker).count();
    if len < 3 {
        return None;
    }
    let info = trim_markdown_line_end(&trimmed[len..]);
    if marker == b'`' && info.as_bytes().contains(&b'`') {
        return None;
    }
    Some((leading, marker, len))
}

fn markdown_fence_closer(line: &str, marker: u8, opener_len: usize) -> Option<(usize, usize)> {
    let leading = leading_markdown_spaces(line);
    if leading > 3 {
        return None;
    }
    let trimmed = &line[leading..];
    let closing_len = trimmed.bytes().take_while(|byte| *byte == marker).count();
    if closing_len < opener_len {
        return None;
    }
    trim_markdown_line_end(&trimmed[closing_len..])
        .is_empty()
        .then_some((leading, closing_len))
}

fn leading_markdown_spaces(line: &str) -> usize {
    line.bytes().take_while(|byte| *byte == b' ').count()
}

fn trim_markdown_line_end(line: &str) -> &str {
    line.trim_end_matches([' ', '\t', '\r', '\n'])
}

fn heading_level(line: &str) -> usize {
    heading_key(line).map_or(0, |(level, _)| level)
}

fn heading_key(line: &str) -> Option<(usize, String)> {
    let leading = leading_markdown_spaces(line);
    if leading > 3 {
        return None;
    }
    let trimmed = line[leading..].trim_end();
    let level = trimmed
        .chars()
        .take_while(|character| *character == '#')
        .count();
    if level == 0 || level > 6 {
        return None;
    }
    let Some(next) = trimmed.chars().nth(level) else {
        return None;
    };
    if !next.is_whitespace() {
        return None;
    }
    let text = strip_closing_heading_hashes(trimmed[level..].trim())
        .trim_end()
        .to_owned();
    Some((level, text))
}

fn strip_closing_heading_hashes(text: &str) -> &str {
    let trimmed = text.trim_end();
    let closing_start = trimmed.trim_end_matches('#').len();
    if closing_start == trimmed.len() || closing_start == 0 {
        return text;
    }
    if trimmed[..closing_start]
        .chars()
        .next_back()
        .is_some_and(char::is_whitespace)
    {
        &trimmed[..closing_start]
    } else {
        text
    }
}

fn replace_text(
    content: &str,
    old_text: &str,
    replacement: &str,
    path: &SkillPath,
) -> Result<String, Trace2SkillPatchError> {
    let count = overlapping_match_count(content, old_text);
    if count == 0 {
        return Err(Trace2SkillPatchError::TextNotFound {
            path: path.to_string(),
            text: old_text.to_owned(),
        });
    }
    if count > 1 {
        return Err(Trace2SkillPatchError::AmbiguousText {
            path: path.to_string(),
            text: old_text.to_owned(),
            matches: count,
        });
    }
    Ok(content.replacen(old_text, replacement, 1))
}

fn overlapping_match_count(content: &str, needle: &str) -> usize {
    content
        .char_indices()
        .filter(|(index, _)| content[*index..].starts_with(needle))
        .count()
}

fn skill_md_reference_links_for_edit(
    before: &str,
    after: &str,
    touched_reference_paths: &BTreeSet<SkillReferencePath>,
) -> Vec<SkillReferencePath> {
    let before_links: BTreeSet<_> = SkillReferencePath::extract_from_text(before)
        .into_iter()
        .collect();
    SkillReferencePath::extract_from_text(after)
        .into_iter()
        .filter(|reference| {
            !before_links.contains(reference) || touched_reference_paths.contains(reference)
        })
        .collect()
}

fn split_lines(content: &str) -> Vec<String> {
    content.split('\n').map(str::to_owned).collect()
}

fn prefixed_block(content: &str) -> Vec<String> {
    let mut block = vec![String::new()];
    block.extend(split_lines(content));
    block
}

fn require_non_empty_content(
    edit: &UpstreamJsonEdit,
    path: &SkillPath,
) -> Result<(), Trace2SkillPatchError> {
    if edit.content.trim().is_empty() {
        return Err(Trace2SkillPatchError::EmptyPatchContent {
            path: path.to_string(),
            op: edit.op.clone(),
        });
    }
    Ok(())
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
    /// More than one fenced JSON block parsed as an upstream patch.
    #[error("Trace2Skill patch contains multiple parseable fenced json blocks")]
    AmbiguousJsonFence,
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
    /// A section-based operation found the heading more than once.
    #[error("Trace2Skill patch found ambiguous section {section:?} in {path}")]
    AmbiguousSection {
        /// Target path.
        path: String,
        /// Ambiguous heading.
        section: String,
    },
    /// Section creation target was not markdown heading syntax.
    #[error("Trace2Skill patch section target {section:?} in {path} is not a markdown heading")]
    InvalidSectionHeading {
        /// Target path.
        path: String,
        /// Invalid heading.
        section: String,
    },
    /// Section creation targets a heading that already exists.
    #[error("Trace2Skill patch would create duplicate section {section:?} in {path}")]
    DuplicateSection {
        /// Target path.
        path: String,
        /// Duplicate heading.
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
    /// Patch operation omitted content that must be non-empty.
    #[error("Trace2Skill patch {op} op for {path} has empty content")]
    EmptyPatchContent {
        /// Target path.
        path: String,
        /// Operation name.
        op: String,
    },
    /// Section-scoped operation lacks a translated target section.
    #[error("Trace2Skill patch {op} op for {path} has empty target_section")]
    MissingTargetSection {
        /// Target path.
        path: String,
        /// Operation name.
        op: String,
    },
    /// Section creation lacks an explicit target heading.
    #[error("Trace2Skill patch {op} op for {path} has empty target_section")]
    EmptyTargetSection {
        /// Target path.
        path: String,
        /// Operation name.
        op: String,
    },
    /// Exact target text was not found after translation.
    #[error("Trace2Skill patch could not find text {text:?} in {path}")]
    TextNotFound {
        /// Target path.
        path: String,
        /// Missing text.
        text: String,
    },
    /// Exact target text appears more than once.
    #[error("Trace2Skill patch found {matches} matches for text {text:?} in {path}")]
    AmbiguousText {
        /// Target path.
        path: String,
        /// Ambiguous text.
        text: String,
        /// Number of matches.
        matches: usize,
    },
    /// Parsed patch lowering or plan validation failed.
    #[error(transparent)]
    ParsedPatch(#[from] SkillParsedPatchError),
    /// Atomic skill application failed.
    #[error(transparent)]
    Application(#[from] SkillPatchApplicationError),
}
