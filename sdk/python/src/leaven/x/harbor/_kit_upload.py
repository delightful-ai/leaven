"""Upload a staged AgentKit into a Harbor environment working directory.

Harbor's default Docker ``upload_file`` is ``docker compose cp`` and does not
create missing parent directories. Repo-scope kits therefore must mkdir the
prompt and nested skill parents before each upload, or live trials fail while
staging ``.agents/skills/<name>/...`` (and the Claude Code equivalent).
"""

from collections.abc import Sequence
from pathlib import Path, PurePosixPath
from typing import Protocol

from leaven.x.harbor._kit import KIT_PROMPT_FILE, KIT_SKILLS_DIR


class KitUploadEnvironment(Protocol):
    """Harbor environment surface needed to place a staged kit in a workdir."""

    async def ensure_dirs(
        self, dirs: Sequence[str], *, chmod: bool = True
    ) -> object | None: ...

    async def upload_file(self, source_path: Path | str, target_path: str) -> None: ...


async def upload_kit_tree(
    environment: KitUploadEnvironment,
    *,
    kit_dir: Path,
    workdir: str,
    prompt_file: str,
    skills_subdir: str,
) -> None:
    """Upload a staged kit into ``<workdir>`` as the agent's project surface."""
    root = PurePosixPath(workdir)
    uploads: list[tuple[Path, PurePosixPath]] = []

    prompt = kit_dir / KIT_PROMPT_FILE
    if prompt.is_file():
        uploads.append((prompt, root / prompt_file))

    skills_root = kit_dir / KIT_SKILLS_DIR
    if skills_root.is_dir():
        for skill_file in sorted(skills_root.rglob("*")):
            if not skill_file.is_file():
                continue
            relative = skill_file.relative_to(skills_root)
            target = root / skills_subdir / PurePosixPath(relative.as_posix())
            uploads.append((skill_file, target))

    if not uploads:
        return

    parents = sorted({str(target.parent) for _, target in uploads})
    # Docker compose cp refuses nested destinations whose parents are missing.
    await environment.ensure_dirs(parents, chmod=False)
    for source, target in uploads:
        await environment.upload_file(source, target.as_posix())


__all__ = ["KitUploadEnvironment", "upload_kit_tree"]
