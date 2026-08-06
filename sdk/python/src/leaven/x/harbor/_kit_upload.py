"""Upload a staged AgentKit into a Harbor environment working directory.

Harbor's default Docker ``upload_file`` is ``docker compose cp`` and does not
create missing parent directories. Repo-scope kits therefore must mkdir the
prompt and nested skill parents before each upload.

Claude Code project skills only load ``<workdir>/.claude/skills/<n>/SKILL.md``
packages. Portable AgentKit skill paths such as ``regex/notes.md`` must be
projected into that package shape; a path-preserving copy is silently ignored.
"""

from collections.abc import Sequence
from pathlib import Path, PurePosixPath
from typing import Literal, Protocol

from leaven.x.harbor._kit import KIT_PROMPT_FILE, KIT_SKILLS_DIR
from leaven.x.harbor._types import HarborAdapterError

SkillsLayout = Literal["portable", "claude_packages"]
CLAUDE_PROJECTION_DIR = ".leaven_claude_skill_projection"


class KitUploadEnvironment(Protocol):
    """Harbor environment surface needed to place a staged kit in a workdir."""

    async def ensure_dirs(self, dirs: Sequence[str], *, chmod: bool = True) -> object | None: ...

    async def upload_file(self, source_path: Path | str, target_path: str) -> None: ...


async def upload_kit_tree(
    environment: KitUploadEnvironment,
    *,
    kit_dir: Path,
    workdir: str,
    prompt_file: str,
    skills_subdir: str,
    skills_layout: SkillsLayout = "portable",
) -> None:
    """Upload a staged kit into ``<workdir>`` as the agent's project surface."""
    root = PurePosixPath(workdir)
    uploads: list[tuple[Path, PurePosixPath]] = []

    prompt = kit_dir / KIT_PROMPT_FILE
    if prompt.is_file():
        uploads.append((prompt, root / prompt_file))

    skills_root = kit_dir / KIT_SKILLS_DIR
    if skills_root.is_dir():
        if skills_layout == "claude_packages":
            uploads.extend(
                _claude_skill_uploads(
                    skills_root=skills_root,
                    dest_skills=root / skills_subdir,
                    projection_root=kit_dir / CLAUDE_PROJECTION_DIR,
                )
            )
        else:
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


def _claude_skill_uploads(
    *,
    skills_root: Path,
    dest_skills: PurePosixPath,
    projection_root: Path,
) -> list[tuple[Path, PurePosixPath]]:
    """Project staged AgentKit skills into Claude Code ``<n>/SKILL.md`` packages."""
    uploads: list[tuple[Path, PurePosixPath]] = []
    package_dirs = sorted(
        {path.parent.resolve() for path in skills_root.rglob("SKILL.md") if path.is_file()},
        key=lambda path: path.as_posix(),
    )
    used_names: dict[str, str] = {}

    for package_dir in package_dirs:
        name = package_dir.name
        source_label = package_dir.relative_to(skills_root).as_posix()
        _claim_claude_skill_name(used_names, name=name, source=source_label)
        for skill_file in sorted(package_dir.rglob("*")):
            if not skill_file.is_file():
                continue
            relative = skill_file.relative_to(package_dir)
            uploads.append((skill_file, dest_skills / name / PurePosixPath(relative.as_posix())))

    package_prefixes = package_dirs
    for skill_file in sorted(skills_root.rglob("*")):
        if not skill_file.is_file():
            continue
        resolved = skill_file.resolve()
        if any(
            resolved == package_dir or package_dir in resolved.parents
            for package_dir in package_prefixes
        ):
            continue
        relative = skill_file.relative_to(skills_root)
        name = _claude_skill_slug(relative)
        _claim_claude_skill_name(used_names, name=name, source=relative.as_posix())
        projected = _write_projected_claude_skill(
            projection_root=projection_root,
            name=name,
            source_path=relative.as_posix(),
            body=skill_file.read_text(encoding="utf-8"),
        )
        uploads.append((projected, dest_skills / name / "SKILL.md"))

    return uploads


def _claude_skill_slug(relative: PurePosixPath) -> str:
    """Derive a Claude skill directory name from a portable AgentKit skill path."""
    stem_parts: list[str] = []
    for part in relative.parts:
        token = part[:-3] if part.endswith(".md") else part
        stem_parts.append(token.replace("_", "-"))
    slug = "-".join(part for part in stem_parts if part)
    if not slug or slug.startswith(".") or "/" in slug or "\\" in slug:
        raise HarborAdapterError(
            f"cannot project AgentKit skill path {relative.as_posix()!r} into a "
            "Claude Code skill package name"
        )
    return slug.lower()


def _claim_claude_skill_name(used_names: dict[str, str], *, name: str, source: str) -> None:
    prior = used_names.get(name)
    if prior is not None:
        raise HarborAdapterError(
            f"Claude Code skill package name {name!r} collides for AgentKit paths "
            f"{prior!r} and {source!r}; rename one skill before repo placement"
        )
    used_names[name] = source


def _write_projected_claude_skill(
    *,
    projection_root: Path,
    name: str,
    source_path: str,
    body: str,
) -> Path:
    package_dir = projection_root / name
    package_dir.mkdir(parents=True, exist_ok=True)
    target = package_dir / "SKILL.md"
    target.write_text(
        _as_claude_skill_md(name=name, source_path=source_path, body=body),
        encoding="utf-8",
    )
    return target


def _as_claude_skill_md(*, name: str, source_path: str, body: str) -> str:
    if body.lstrip().startswith("---"):
        return body
    description = f"Leaven AgentKit skill ({source_path})."
    return f"---\nname: {name}\ndescription: {description}\n---\n\n{body}"


__all__ = ["KitUploadEnvironment", "SkillsLayout", "upload_kit_tree"]
