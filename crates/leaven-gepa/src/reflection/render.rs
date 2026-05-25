use std::fmt::Write as _;

use leaven_core::OptimizationProblem;
use leaven_engine::ProposalError;
use leaven_surface::EditSurface;

use super::{
    ReflectionRenderInput, ReflectiveCase, ReflectiveRun, ReflectiveSideInfoValue, ReflectiveValue,
};

pub(super) fn selected_part_view<P, S>(
    input: &ReflectionRenderInput<'_, P, S>,
) -> Result<String, ProposalError>
where
    P: OptimizationProblem,
    S: EditSurface<P::Artifact>,
    S::PartId: std::fmt::Debug,
    for<'a> S::View<'a>: std::fmt::Display,
{
    let parts = input.surface.parts(input.artifact).map_err(|source| {
        ProposalError::with_source("GEPA reflection surface projection failed", source)
    })?;
    let part = parts
        .into_iter()
        .find(|part| part.id == input.request.part)
        .ok_or_else(|| {
            ProposalError::Message(format!(
                "selected GEPA reflection part {:?} is missing from surface",
                input.request.part
            ))
        })?;
    Ok(part.view.to_string())
}

pub(super) fn render_reflective_cases(cases: &[ReflectiveCase]) -> String {
    if cases.is_empty() {
        return "(no reflective examples were selected)".to_owned();
    }

    cases
        .iter()
        .enumerate()
        .map(|(index, case)| {
            let mut rendered = String::new();
            let _ = writeln!(rendered, "# Example {}", index + 1);
            if case.runs.is_empty() {
                render_reflective_case_without_runs(&mut rendered, case);
            } else if case.runs.len() == 1 {
                render_reflective_case_sections(&mut rendered, case, &case.runs[0]);
            } else {
                render_reflective_case_context(&mut rendered, case);
                for (run_index, run) in case.runs.iter().enumerate() {
                    let _ = writeln!(rendered, "## Run {}", run_index + 1);
                    render_reflective_run_sections(&mut rendered, run);
                }
            }
            rendered
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn render_reflective_case_without_runs(rendered: &mut String, case: &ReflectiveCase) {
    render_reflective_case_context(rendered, case);
    if !case.source_refs.is_empty() {
        let _ = writeln!(rendered, "## Source refs\n{:?}", case.source_refs);
    }
}

fn render_reflective_case_context(rendered: &mut String, case: &ReflectiveCase) {
    if let Some(case_id) = case.case_id {
        let _ = writeln!(rendered, "## Case\n{case_id}");
    }
    if let Some(input) = render_reflective_value(&case.input) {
        let _ = writeln!(rendered, "## Input\n{}", input.trim());
    }
    if let Some(expected) = &case.expected
        && let Some(expected) = render_reflective_value(expected)
    {
        let _ = writeln!(rendered, "## Expected\n{}", expected.trim());
    }
}

fn render_reflective_case_sections(
    rendered: &mut String,
    case: &ReflectiveCase,
    run: &ReflectiveRun,
) {
    if !run.side_info.is_empty() {
        for (name, value) in &run.side_info {
            let _ = writeln!(rendered, "## {}", name.trim());
            render_side_info_value(rendered, value, 3);
        }
        return;
    }
    if let Some(case_id) = case.case_id {
        let _ = writeln!(rendered, "## Case\n{case_id}");
    }
    if let Some(input) = render_reflective_value(&case.input) {
        let _ = writeln!(rendered, "## Input\n{}", input.trim());
    }
    if let Some(expected) = &case.expected
        && let Some(expected) = render_reflective_value(expected)
    {
        let _ = writeln!(rendered, "## Expected\n{}", expected.trim());
    }
    render_reflective_run_sections(rendered, run);
}

fn render_reflective_run_sections(rendered: &mut String, run: &ReflectiveRun) {
    if !run.side_info.is_empty() {
        for (name, value) in &run.side_info {
            let _ = writeln!(rendered, "### {}", name.trim());
            render_side_info_value(rendered, value, 4);
        }
        return;
    }
    if let Some(score) = run.score {
        let _ = writeln!(rendered, "## Score\n{score}");
    }
    if let Some(output) = run.produced.as_ref().and_then(render_reflective_value) {
        let _ = writeln!(rendered, "## Output\n{}", output.trim());
    }
    if !run.feedback.is_empty() {
        let _ = writeln!(rendered, "## Feedback\n{}", run.feedback.trim());
    }
    rendered.push('\n');
}

fn render_reflective_value(value: &ReflectiveValue) -> Option<String> {
    match value {
        ReflectiveValue::Text(text) if text.is_empty() => None,
        ReflectiveValue::Text(text) => Some(text.clone()),
        ReflectiveValue::Json(value) => Some(value.to_string()),
        ReflectiveValue::File(reference) => {
            Some(format!("trace:{}:{}", reference.store, reference.key))
        }
        ReflectiveValue::Mapping(fields) if fields.is_empty() => None,
        ReflectiveValue::Mapping(fields) => Some(
            fields
                .iter()
                .map(|(key, value)| {
                    let rendered = render_reflective_value(value).unwrap_or_default();
                    format!("{key}: {rendered}")
                })
                .collect::<Vec<_>>()
                .join("\n"),
        ),
    }
}

fn render_side_info_value(rendered: &mut String, value: &ReflectiveSideInfoValue, level: usize) {
    match value {
        ReflectiveSideInfoValue::Text(text) => {
            let _ = writeln!(rendered, "{}\n", text.trim());
        }
        ReflectiveSideInfoValue::Mapping(fields) => {
            for (name, value) in fields {
                let _ = writeln!(rendered, "{} {}", "#".repeat(level), name.trim());
                render_side_info_value(rendered, value, (level + 1).min(6));
            }
            if fields.is_empty() {
                rendered.push('\n');
            }
        }
        ReflectiveSideInfoValue::List(items) => {
            for (index, value) in items.iter().enumerate() {
                let _ = writeln!(rendered, "{} Item {}", "#".repeat(level), index + 1);
                render_side_info_value(rendered, value, (level + 1).min(6));
            }
            if items.is_empty() {
                rendered.push('\n');
            }
        }
    }
}

pub(super) fn render_prompt_template(
    template: &str,
    current_instruction: &str,
    side_info: &str,
) -> Result<String, ProposalError> {
    let missing = ["<curr_param>", "<side_info>"]
        .into_iter()
        .filter(|placeholder| !template.contains(placeholder))
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        return Err(ProposalError::Message(format!(
            "GEPA reflection prompt template is missing placeholder(s): {}",
            missing.join(", ")
        )));
    }

    Ok(template
        .replace("<curr_param>", current_instruction)
        .replace("<side_info>", side_info))
}

pub(super) fn extract_replacement_text(assistant_text: &str) -> String {
    let text = assistant_text.trim();
    let Some(start) = text.find("```") else {
        return text.to_owned();
    };
    let content_start = start + 3;
    let Some(end) = text.rfind("```").filter(|end| *end >= content_start) else {
        return strip_opening_fence(text);
    };
    if end == start {
        return strip_opening_fence(text);
    }

    let fenced = &text[content_start..end];
    strip_optional_language(fenced).trim().to_owned()
}

fn strip_opening_fence(text: &str) -> String {
    text.strip_prefix("```")
        .map(strip_optional_language)
        .unwrap_or(text)
        .trim()
        .trim_end_matches("```")
        .trim()
        .to_owned()
}

pub(super) fn strip_optional_language(text: &str) -> &str {
    match text.find('\n') {
        Some(newline) => {
            let first_line = &text[..newline];
            if first_line.is_empty() || !first_line.contains(char::is_whitespace) {
                &text[newline + 1..]
            } else {
                text
            }
        }
        None => text,
    }
}
