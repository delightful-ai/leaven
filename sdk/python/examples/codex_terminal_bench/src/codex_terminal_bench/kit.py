"""Materialize a Leaven agent kit to an on-disk directory for Harbor upload.

The `LeavenCodex` agent uploads the kit from a directory: the system prompt as
`AGENTS.md` and each skill under `skills/<path>`. This module writes an
`AgentKitArtifact` into that layout so the rollout can hand the directory to the
agent through `AgentConfig.kwargs["agent_kit_dir"]`.
"""

from pathlib import Path

import leaven as lv


def materialize_kit(kit: lv.AgentKitArtifact, target_dir: Path) -> Path:
    """Write a kit into `target_dir` as `AGENTS.md` + `skills/<path>`.

    Returns `target_dir`. The directory is created if missing; skill paths are
    treated as relative POSIX paths under `skills/` (the host's `AgentKit` path
    law already rejected absolute paths and parent traversal upstream).
    """
    target_dir.mkdir(parents=True, exist_ok=True)
    (target_dir / "AGENTS.md").write_text(kit.system_prompt, encoding="utf-8")
    if kit.skills:
        skills_root = target_dir / "skills"
        for skill in kit.skills:
            skill_path = skills_root / skill.path
            skill_path.parent.mkdir(parents=True, exist_ok=True)
            skill_path.write_text(skill.content, encoding="utf-8")
    return target_dir


__all__ = ["materialize_kit"]
