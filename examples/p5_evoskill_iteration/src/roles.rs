const EXECUTOR_SOURCE_PROMPT: &str = include_str!("../fixtures/source-prompts/executor.md");
const SKILL_PROPOSER_SOURCE_PROMPT: &str =
    include_str!("../fixtures/source-prompts/skill-proposer.md");
const SKILL_BUILDER_SOURCE_PROMPT: &str =
    include_str!("../fixtures/source-prompts/skill-builder.md");

pub fn executor_developer_instructions() -> String {
    with_leaven_wrapper(
        EXECUTOR_SOURCE_PROMPT,
        [
            "Leaven/Codex live-run wrapper:",
            "- Skills, when present, are mounted at `.agents/skills/<skill-name>/SKILL.md`.",
            "- Load a skill only when its `name` or `description` is relevant to the task.",
            "- This EvoSkill reproduction treats reusable numeric conversion procedures as skill-gated.",
            "- If no relevant skill exists for a specialized reusable procedure, final_answer MUST be exactly `NOT_ATTEMPTED`.",
            "- Do not call tools in this app-server reproduction; the task contains the needed case data inline.",
            "- Reply with JSON only, with string keys `final_answer` and `reasoning`.",
        ],
    )
}

pub fn proposer_developer_instructions() -> String {
    with_leaven_wrapper(
        SKILL_PROPOSER_SOURCE_PROMPT,
        [
            "Leaven/Codex live-run wrapper:",
            "- Existing Leaven Agent Skills are listed in the task and mounted at `.agents/skills`.",
            "- The mandatory brainstorming skill is available at `.claude/skills/brainstorming/SKILL.md`.",
            "- Do not call tools in this app-server reproduction; the task contains failures and skill inventory inline.",
            "- Return the EvoSkill proposer schema as JSON in the final message.",
            "- Do not memorize validation answers or propose one-row benchmark patches.",
        ],
    )
}

pub fn skill_builder_developer_instructions() -> String {
    with_leaven_wrapper(
        SKILL_BUILDER_SOURCE_PROMPT,
        [
            "Leaven/Codex live-run wrapper:",
            "- The active candidate skill bank lives at `.agents/skills`, not `.claude/skills`.",
            "- The mandatory skill-creator skill is available at `.claude/skills/skill-creator/SKILL.md`.",
            "- Do not call tools in this app-server reproduction; return the skill files as JSON in the final message.",
            "- Create or edit exactly one Agent Skill folder under `.agents/skills/<skill-name>/`.",
            "- Every skill must contain a valid `SKILL.md` with `name`, `description`, and non-empty body.",
            "- Skills may contain scripts, references, assets, and any other files.",
            "- The JSON must include `skill_name`, `files`, `generated_skill`, and `reasoning`.",
        ],
    )
}

pub fn brainstorming_meta_skill() -> &'static str {
    r"---
name: brainstorming
description: Explore two or three candidate approaches before choosing a simple reusable skill proposal.
---

Before proposing a skill, list 2-3 approaches. For each, note the core idea,
trade-off, and complexity. Pick the simplest approach that addresses the root
cause without overfitting to one case.
"
}

pub fn skill_creator_meta_skill() -> &'static str {
    r"---
name: skill-creator
description: Author valid Agent Skills folders with concise retrieval metadata and reusable instructions.
---

Write a skill as a directory with `SKILL.md` at its root. The frontmatter must
have `name` and `description`; all other fields belong in generic metadata.
The description should say both what the skill does and when an agent should
use it. Keep the body reusable: define constraints, examples, and any script or
reference files the agent can load on demand. Do not solve one benchmark row by
memorizing its answer.
"
}

fn with_leaven_wrapper<const N: usize>(source: &str, wrapper: [&str; N]) -> String {
    let mut instructions = String::with_capacity(source.len() + 512);
    instructions.push_str(source.trim());
    instructions.push_str("\n\n");
    instructions.push_str(&wrapper.join("\n"));
    instructions
}
