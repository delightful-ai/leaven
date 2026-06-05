"""SkillBank — the EvoSkill paper's optimization target.

A SkillBank is a collection of skill files (markdown), optionally with
shared references. Optimizers evolve the bank by proposing
add/edit/remove/rename operations.

The actual SkillBank semantics live in `leaven-agentic-skill` on the Rust
side; this is the Python projection of the wire type. Schema generation
will eventually replace this hand-written stub with codegen from the
skill bank's locked JSON schema.
"""

import json
from pathlib import Path
from typing import Annotated, Literal, Self

from pydantic import BaseModel, ConfigDict, Field

from .._json_parse import parse_json_object
from ..json_value import JsonObject


class SkillFile(BaseModel):
    """One skill file inside a bank."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    path: str
    """Bank-relative path (e.g. `skills/treasury-notation.md`)."""
    content: str
    """Markdown content."""
    references: list[str] = Field(default_factory=list)
    """Bank-relative paths of files this skill references (e.g. `references/*.md`)."""


class SkillBank(BaseModel):
    """A bank of skill files. Built directly or returned from the engine."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    files: list[SkillFile] = Field(default_factory=list)
    candidate_id: str | None = None
    """Set when the bank came from the engine; None for hand-built seeds."""

    @classmethod
    def empty(cls) -> Self:
        """An empty seed bank."""
        return cls(files=[])

    @classmethod
    def from_directory(cls, path: str) -> Self:
        """Load a bank from a local directory layout (skills/ + references/)."""
        root = Path(path)
        if not root.is_dir():
            raise NotADirectoryError(path)
        reference_paths = _markdown_paths(root / "references", root=root)
        files = [
            SkillFile(
                path=relative_path,
                content=(root / relative_path).read_text(encoding="utf-8"),
                references=list(reference_paths) if relative_path.startswith("skills/") else [],
            )
            for relative_path in sorted(
                [*_markdown_paths(root / "references", root=root), *_markdown_paths(root / "skills", root=root)]
            )
        ]
        return cls(files=files)


class SkillBankChangeRecord(BaseModel):
    """Base class for typed skill-bank mutation records."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    def to_json_value(self) -> JsonObject:
        """Project this typed change to the public-seam JSON literal encoding."""
        return parse_json_object(
            json.loads(self.model_dump_json(by_alias=True, exclude_none=True)),
            context="skill-bank change",
        )


class SkillBankChangeFile(BaseModel):
    """File contents carried by a skill-bank write change."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    content: str
    executable: bool = False


class SkillBankFolder(BaseModel):
    """Complete skill folder value for create/replace changes."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    name: str
    files: list[SkillBankChangeFile] = Field(default_factory=list)


class SkillBankCreateSkillChange(SkillBankChangeRecord):
    """Create a skill folder."""

    kind: Literal["create_skill"] = "create_skill"
    folder: SkillBankFolder


class SkillBankReplaceSkillChange(SkillBankChangeRecord):
    """Replace one skill folder by name."""

    kind: Literal["replace_skill"] = "replace_skill"
    name: str
    folder: SkillBankFolder


class SkillBankRemoveSkillChange(SkillBankChangeRecord):
    """Remove one skill folder."""

    kind: Literal["remove_skill"] = "remove_skill"
    name: str


class SkillBankRenameSkillChange(SkillBankChangeRecord):
    """Rename one skill folder."""

    model_config = ConfigDict(frozen=True, extra="forbid", populate_by_name=True)

    kind: Literal["rename_skill"] = "rename_skill"
    from_name: str = Field(validation_alias="from", serialization_alias="from")
    to: str


class SkillBankWriteFileChange(SkillBankChangeRecord):
    """Write one file inside a skill."""

    kind: Literal["write_file"] = "write_file"
    skill: str
    path: str
    file: SkillBankChangeFile


class SkillBankRemoveFileChange(SkillBankChangeRecord):
    """Remove one file inside a skill."""

    kind: Literal["remove_file"] = "remove_file"
    skill: str
    path: str


class SkillBankRenameFileChange(SkillBankChangeRecord):
    """Rename one file inside a skill."""

    model_config = ConfigDict(frozen=True, extra="forbid", populate_by_name=True)

    kind: Literal["rename_file"] = "rename_file"
    skill: str
    from_path: str = Field(validation_alias="from", serialization_alias="from")
    to: str


class SkillBankSetExecutableChange(SkillBankChangeRecord):
    """Change one skill file's executable bit."""

    kind: Literal["set_executable"] = "set_executable"
    skill: str
    path: str
    executable: bool


class SkillBankAtomicChange(SkillBankChangeRecord):
    """Apply multiple skill-bank changes atomically."""

    kind: Literal["atomic"] = "atomic"
    changes: list["SkillBankChange"]


type SkillBankChange = Annotated[
    SkillBankCreateSkillChange
    | SkillBankReplaceSkillChange
    | SkillBankRemoveSkillChange
    | SkillBankRenameSkillChange
    | SkillBankWriteFileChange
    | SkillBankRemoveFileChange
    | SkillBankRenameFileChange
    | SkillBankSetExecutableChange
    | SkillBankAtomicChange,
    Field(discriminator="kind"),
]


def _markdown_paths(directory: Path, *, root: Path) -> list[str]:
    if not directory.exists():
        return []
    if not directory.is_dir():
        raise NotADirectoryError(str(directory))
    return [
        path.relative_to(root).as_posix()
        for path in directory.rglob("*.md")
        if path.is_file()
    ]


__all__ = [
    "SkillBank",
    "SkillBankAtomicChange",
    "SkillBankChange",
    "SkillBankChangeFile",
    "SkillBankChangeRecord",
    "SkillBankCreateSkillChange",
    "SkillBankFolder",
    "SkillBankRemoveFileChange",
    "SkillBankRemoveSkillChange",
    "SkillBankRenameFileChange",
    "SkillBankRenameSkillChange",
    "SkillBankReplaceSkillChange",
    "SkillBankSetExecutableChange",
    "SkillBankWriteFileChange",
    "SkillFile",
]
