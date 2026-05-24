"""SkillBank — the EvoSkill paper's optimization target.

A SkillBank is a collection of skill files (markdown), optionally with
shared references. Optimizers evolve the bank by proposing
add/edit/remove/rename operations.

The actual SkillBank semantics live in `leaven-agentic-skill` on the Rust
side; this is the Python projection of the wire type. Schema generation
will eventually replace this hand-written stub with codegen from the
skill bank's locked JSON schema.
"""

from __future__ import annotations

from typing import Self

from pydantic import BaseModel, ConfigDict, Field


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
        raise NotImplementedError("scaffold; see docs/specs/leaven_python.md")


__all__ = ["SkillBank", "SkillFile"]
