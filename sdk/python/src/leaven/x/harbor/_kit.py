"""Materialize Leaven AgentKit artifacts for Harbor Codex agents."""

from pathlib import Path


def materialize_agent_kit(kit: object, target_dir: Path) -> Path:
    """Write an AgentKit as `AGENTS.md` plus `skills/<path>` under target_dir."""
    target_dir.mkdir(parents=True, exist_ok=True)
    (target_dir / "AGENTS.md").write_text(kit.system_prompt, encoding="utf-8")
    for skill in kit.skills:
        skill_path = target_dir / "skills" / skill.path
        skill_path.parent.mkdir(parents=True, exist_ok=True)
        skill_path.write_text(skill.content, encoding="utf-8")
    return target_dir


__all__ = ["materialize_agent_kit"]
