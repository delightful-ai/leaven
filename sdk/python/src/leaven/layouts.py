"""Workspace layout declarations for stage objects.

Layouts are stable contracts owned by stages. They name where the runtime should
project artifacts, case assets, instructions, and outputs inside a workspace.
They do not allocate workspaces and do not own artifact or task semantics.
"""

from __future__ import annotations

from typing import Literal

from pydantic import BaseModel, ConfigDict


class WorkspaceLayout(BaseModel):
    """A stage-owned workspace layout contract."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    kind: Literal["case_workspace", "edit_artifact"]
    artifact_root: str = "target/current"
    case_root: str = "case"
    output_root: str = "output"
    mutable_roots: tuple[str, ...] = ("target/current",)
    readonly_roots: tuple[str, ...] = ("case", "TASK.md", ".leaven")


def case_workspace() -> WorkspaceLayout:
    """Layout for running the current artifact against one sample."""
    return WorkspaceLayout(kind="case_workspace")


def edit_artifact() -> WorkspaceLayout:
    """Layout for agentic proposal stages that edit the artifact in place."""
    return WorkspaceLayout(
        kind="edit_artifact",
        case_root="cases",
        output_root="output",
        mutable_roots=("target/current",),
        readonly_roots=("cases", "cross_case", "TASK.md", "AGENTS.md", ".leaven"),
    )


__all__ = ["WorkspaceLayout", "case_workspace", "edit_artifact"]
