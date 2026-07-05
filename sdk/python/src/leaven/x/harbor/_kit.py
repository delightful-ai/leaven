"""Materialize Leaven AgentKit artifacts into a neutral staging layout.

A kit is staged as `<dir>/AGENTS.md` (the system prompt) plus `<dir>/skills/<path>`
(skill files). Adapters in `agents.py` map this neutral layout onto each Harbor
agent's real configuration surface: user-scope config (append-system-prompt /
`$CODEX_HOME/AGENTS.md` + `AgentConfig.skills`) or repo-scope working-dir files
(`<workdir>/CLAUDE.md` or `<workdir>/AGENTS.md` + the agent's skills subtree).
"""

from pathlib import Path

from leaven.x.harbor._types import HarborAdapterError

# Neutral staging names. The system prompt is staged as AGENTS.md (the kit's
# canonical instruction file); adapters rename it per agent when uploading
# (Codex keeps AGENTS.md, Claude Code uploads it as CLAUDE.md).
KIT_PROMPT_FILE = "AGENTS.md"
KIT_SKILLS_DIR = "skills"


def materialize_agent_kit(kit: object, target_dir: Path) -> Path:
    """Stage an AgentKit as `AGENTS.md` plus `skills/<path>` under target_dir."""
    staged_skills = [(_normalize_skill_path(skill.path), skill.content) for skill in kit.skills]
    target_dir.mkdir(parents=True, exist_ok=True)
    (target_dir / KIT_PROMPT_FILE).write_text(kit.system_prompt, encoding="utf-8")
    for relative_path, content in staged_skills:
        skill_path = target_dir / KIT_SKILLS_DIR / relative_path
        skill_path.parent.mkdir(parents=True, exist_ok=True)
        skill_path.write_text(content, encoding="utf-8")
    return target_dir


def _normalize_skill_path(value: object) -> str:
    if not isinstance(value, str):
        raise HarborAdapterError("AgentKit skill path must be a string")
    if value.startswith("/"):
        raise HarborAdapterError("AgentKit skill path must be relative")
    if "\\" in value:
        raise HarborAdapterError("AgentKit skill path must be a portable POSIX path")
    if "\0" in value:
        raise HarborAdapterError("AgentKit skill path must not contain NUL")

    normalized = value.rstrip("/")
    if normalized == "":
        raise HarborAdapterError("AgentKit skill path must not be empty")
    for component in normalized.split("/"):
        if component == "":
            raise HarborAdapterError("AgentKit skill path must not contain empty components")
        if component == ".":
            raise HarborAdapterError("AgentKit skill path must not contain current-directory components")
        if component == "..":
            raise HarborAdapterError("AgentKit skill path must not contain parent traversal")
    return normalized


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
