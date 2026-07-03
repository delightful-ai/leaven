"""Materialize Leaven AgentKit artifacts into a neutral staging layout.

A kit is staged as `<dir>/AGENTS.md` (the system prompt) plus `<dir>/skills/<path>`
(skill files). Adapters in `agents.py` map this neutral layout onto each Harbor
agent's real configuration surface: user-scope config (append-system-prompt /
`$CODEX_HOME/AGENTS.md` + `AgentConfig.skills`) or repo-scope working-dir files
(`<workdir>/CLAUDE.md` or `<workdir>/AGENTS.md` + the agent's skills subtree).
"""

from pathlib import Path

# Neutral staging names. The system prompt is staged as AGENTS.md (the kit's
# canonical instruction file); adapters rename it per agent when uploading
# (Codex keeps AGENTS.md, Claude Code uploads it as CLAUDE.md).
KIT_PROMPT_FILE = "AGENTS.md"
KIT_SKILLS_DIR = "skills"


def materialize_agent_kit(kit: object, target_dir: Path) -> Path:
    """Stage an AgentKit as `AGENTS.md` plus `skills/<path>` under target_dir."""
    target_dir.mkdir(parents=True, exist_ok=True)
    (target_dir / KIT_PROMPT_FILE).write_text(kit.system_prompt, encoding="utf-8")
    for skill in kit.skills:
        skill_path = _staged_skill_path(target_dir, skill.path)
        skill_path.parent.mkdir(parents=True, exist_ok=True)
        skill_path.write_text(skill.content, encoding="utf-8")
    return target_dir


def _staged_skill_path(target_dir: Path, path: object) -> Path:
    if not isinstance(path, str):
        raise TypeError("agent_kit skill path must be a string")

    skills_root = target_dir.resolve(strict=True) / KIT_SKILLS_DIR
    skill_path = (target_dir / KIT_SKILLS_DIR / path).resolve(strict=False)
    if not skill_path.is_relative_to(skills_root):
        raise ValueError("agent_kit skill path escapes the staging skills directory")
    return skill_path


def staged_prompt(target_dir: Path) -> str:
    """The staged system-prompt text (empty string if absent)."""
    prompt = target_dir / KIT_PROMPT_FILE
    return prompt.read_text(encoding="utf-8") if prompt.is_file() else ""


def staged_skill_paths(target_dir: Path) -> list[Path]:
    """Top-level skill entries under the staging skills dir (for `AgentConfig.skills`)."""
    skills_root = target_dir / KIT_SKILLS_DIR
    if not skills_root.is_dir():
        return []
    return sorted(skills_root.iterdir())


__all__ = [
    "KIT_PROMPT_FILE",
    "KIT_SKILLS_DIR",
    "materialize_agent_kit",
    "staged_prompt",
    "staged_skill_paths",
]
