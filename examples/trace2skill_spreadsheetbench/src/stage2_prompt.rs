use std::{
    fmt::Write as _,
    fs,
    path::{Path, PathBuf},
};

use leaven_evidence::{AgentAnalystRole, AgentTrajectoryOutcome};

use crate::Trace2SkillManifestError;

pub struct Stage2PromptLine {
    pub label: &'static str,
    pub value: String,
}

#[derive(Clone, Copy)]
pub struct Stage2AnalystPromptInput<'a> {
    pub call_id: &'a str,
    pub task_id: &'a str,
    pub outcome: &'a AgentTrajectoryOutcome,
    pub upstream_prompt_dir: &'a Path,
    pub prompt_source_paths: &'a [PathBuf],
    pub extra_lines: &'a [Stage2PromptLine],
    pub final_instruction: &'a str,
}

pub fn stage2_prompt_source_paths(
    upstream_prompt_dir: &Path,
    outcome: &AgentTrajectoryOutcome,
) -> Vec<PathBuf> {
    let mut relative_paths = vec![
        "skill_evolving_agent/system_prompt_base.txt",
        "parallel_evolving_agent/map_output_format.txt",
    ];
    match outcome {
        AgentTrajectoryOutcome::Success => {
            relative_paths.extend([
                "success_evolving_agent/success_record_section.txt",
                "success_evolving_agent/success_modification_strategies_section.txt",
                "success_evolving_agent/success_intro_replacement.txt",
                "success_evolving_agent/success_input_replacement.txt",
                "success_evolving_agent/success_goal_replacement.txt",
                "success_evolving_agent/success_first_constraint_replacement.txt",
                "success_evolving_agent/success_traceability_constraint.txt",
                "success_evolving_agent/success_output_reasoning_replacement.txt",
                "success_evolving_agent/success_analysis_records_header.txt",
                "success_evolving_agent/current_skill_folder_header.txt",
                "success_evolving_agent/skill_folder_size_status_header.txt",
                "success_evolving_agent/skill_md_status_line.txt",
                "success_evolving_agent/reference_files_status_line.txt",
                "success_evolving_agent/size_warning.txt",
            ]);
        }
        AgentTrajectoryOutcome::Failure { .. } => {
            relative_paths.extend([
                "skill_evolving_agent/modification_strategies_section.txt",
                "skill_evolving_agent/error_record_section_skill.txt",
                "skill_evolving_agent/error_analysis_records_header.txt",
                "skill_evolving_agent/current_skill_folder_header.txt",
                "skill_evolving_agent/skill_folder_size_status_header.txt",
                "skill_evolving_agent/skill_md_status_line.txt",
                "skill_evolving_agent/reference_files_status_line.txt",
                "skill_evolving_agent/size_warning.txt",
            ]);
        }
    }
    relative_paths
        .into_iter()
        .map(|relative| upstream_prompt_dir.join(relative))
        .collect()
}

pub fn render_stage2_analyst_prompt(
    input: Stage2AnalystPromptInput<'_>,
) -> Result<String, Trace2SkillManifestError> {
    let (builder, user_message_builder) = stage2_prompt_builders(input.outcome);
    let mut prompt = format!(
        "# Trace2Skill Stage 2 MAP Analyst Prompt Source\n\n\
         This pending fan-out has not executed an analyst model call. It records the upstream \
         prompt-template material and the scored trajectory artifacts needed for the later live \
         Trace2Skill Stage 2 MAP call.\n\n\
         ## Call\n\
         - call_id: {call_id}\n\
         - task_id: {task_id}\n\
         - role: {role:?}\n\
         - upstream_system_builder: {builder}\n\
         - upstream_user_message_builder: {user_message_builder}\n\
         - upstream_prompt_dir: {prompt_dir}\n",
        call_id = input.call_id,
        task_id = input.task_id,
        role = stage2_analyst_role(input.outcome),
        prompt_dir = input.upstream_prompt_dir.display(),
    );
    for line in input.extra_lines {
        writeln!(&mut prompt, "- {}: {}", line.label, line.value)
            .expect("writing to a String cannot fail");
    }
    prompt.push_str("\n## Source Templates\n");
    for path in input.prompt_source_paths {
        let relative = path.strip_prefix(input.upstream_prompt_dir).unwrap_or(path);
        let contents = fs::read_to_string(path)?;
        write!(
            &mut prompt,
            "\n### {}\n\n```text\n{}\n```\n",
            relative.display(),
            contents
        )
        .expect("writing to a String cannot fail");
    }
    prompt.push_str("\n## Leaven Inputs\n\n");
    prompt.push_str(input.final_instruction);
    prompt.push('\n');
    Ok(prompt)
}

pub fn stage2_analyst_role(outcome: &AgentTrajectoryOutcome) -> AgentAnalystRole {
    match outcome {
        AgentTrajectoryOutcome::Success => AgentAnalystRole::Success,
        AgentTrajectoryOutcome::Failure { .. } => AgentAnalystRole::Error,
    }
}

fn stage2_prompt_builders(outcome: &AgentTrajectoryOutcome) -> (&'static str, &'static str) {
    match outcome {
        AgentTrajectoryOutcome::Success => (
            "skill_evolver.parallel_success_evolving_agent.SuccessParallelSkillEvolver._build_map_system_prompt",
            "skill_evolver.success_evolving_agent.build_success_user_message",
        ),
        AgentTrajectoryOutcome::Failure { .. } => (
            "skill_evolver.parallel_evolving_agent.ParallelSkillEvolver._build_map_system_prompt",
            "skill_evolver.skill_evolving_agent.SkillEvolver.build_user_message",
        ),
    }
}
