"""`cx.workspace.*` — materialize workspaces, read files, run git queries."""

from __future__ import annotations

from collections.abc import Sequence
from typing import Any

from pydantic import BaseModel, ConfigDict

from .._handles import WorkspaceHandle, WorkspaceLifetime, WorkspaceSurface
from .._receipts import CallReceipt, QueryReceipt


class WorkspaceFile(BaseModel):
    """Result of `cx.workspace.read_file(...)`."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    path: str
    content: str | bytes
    receipt: QueryReceipt


class WorkspaceDiff(BaseModel):
    """Result of `cx.workspace.git_diff(...)` and friends."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    text: str
    """Diff text in unified format."""
    receipt: QueryReceipt


class WorkspaceStatus(BaseModel):
    """Result of `cx.workspace.git_status(...)`."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    entries: list[dict[str, Any]]
    """Per-file status entries (porcelain v2 shape)."""
    receipt: QueryReceipt


class WorkspaceListing(BaseModel):
    """Result of `cx.workspace.list(...)`."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    entries: list[dict[str, Any]]
    receipt: QueryReceipt


class WorkspaceSnapshot(BaseModel):
    """Result of `cx.workspace.snapshot(...)` / `cx.workspace.digest(...)`."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    digest: str
    algorithm: str
    receipt: QueryReceipt


class WorkspaceBuilder:
    """Workspace operations bound to a context."""

    async def materialize_candidate(
        self,
        candidate_id: str,
        *,
        surface: WorkspaceSurface = "full_repo",
        lifetime: WorkspaceLifetime = "stage_call",
    ) -> WorkspaceHandle:
        """Materialize a candidate's artifact into a fresh workspace.

        Returned handle auto-releases at the end of the stage_call (or run, or
        manually). Pass the handle into downstream `cx.sandbox.exec` /
        `cx.agent.run` / `cx.workspace.*` calls.
        """
        raise NotImplementedError("scaffold; see docs/specs/leaven_python.md")

    async def release(self, handle: WorkspaceHandle) -> None:
        """Explicit release; only needed for `lifetime='manual'` handles."""
        raise NotImplementedError("scaffold; see docs/specs/leaven_python.md")

    async def read_file(
        self,
        handle: WorkspaceHandle,
        path: str,
        *,
        max_bytes: int | None = None,
    ) -> WorkspaceFile:
        """Read one workspace-relative file."""
        raise NotImplementedError("scaffold; see docs/specs/leaven_python.md")

    async def write_file(
        self,
        handle: WorkspaceHandle,
        path: str,
        content: str | bytes,
    ) -> CallReceipt:
        """Write a workspace-relative file. Receipt binds the change."""
        raise NotImplementedError("scaffold; see docs/specs/leaven_python.md")

    async def list(
        self,
        handle: WorkspaceHandle,
        path: str = ".",
        *,
        recursive: bool = False,
        max_entries: int | None = None,
    ) -> WorkspaceListing:
        """List directory contents under the workspace-relative path."""
        raise NotImplementedError("scaffold; see docs/specs/leaven_python.md")

    async def snapshot(
        self,
        handle: WorkspaceHandle,
        *,
        algorithm: str = "blake3",
    ) -> WorkspaceSnapshot:
        """Whole-workspace content digest."""
        raise NotImplementedError("scaffold; see docs/specs/leaven_python.md")

    async def git_diff(
        self,
        handle: WorkspaceHandle,
        *,
        against: str = "parent",
        expected_data_classes: Sequence[str] | None = None,
    ) -> WorkspaceDiff:
        """Git diff against a ref."""
        raise NotImplementedError("scaffold; see docs/specs/leaven_python.md")

    async def git_status(self, handle: WorkspaceHandle) -> WorkspaceStatus:
        """Git status (porcelain v2)."""
        raise NotImplementedError("scaffold; see docs/specs/leaven_python.md")

    async def git_log(self, handle: WorkspaceHandle) -> WorkspaceDiff:
        """Git log as a workspace_diff-family value (broad projection per seam)."""
        raise NotImplementedError("scaffold; see docs/specs/leaven_python.md")

    async def write_skills(self, handle: WorkspaceHandle, bank: Any) -> CallReceipt:
        """Skill-bank convenience: write a SkillBank into the workspace layout.

        Equivalent to walking the bank and calling `write_file` per file.
        Exists because the skill-bank → workspace mapping is fixed and common.
        """
        raise NotImplementedError("scaffold; see docs/specs/leaven_python.md")


__all__ = [
    "WorkspaceBuilder",
    "WorkspaceDiff",
    "WorkspaceFile",
    "WorkspaceListing",
    "WorkspaceSnapshot",
    "WorkspaceStatus",
]
