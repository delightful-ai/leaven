"""Handle types — opaque references to engine-owned resources.

Workspaces, candidates, runs — Python users hold typed handles, never raw
ids. Handle lifetimes are engine-enforced (`stage_call`, `run`, `manual`).
"""

from __future__ import annotations

from typing import Literal

from pydantic import BaseModel, ConfigDict

from ._receipts import CallReceipt

WorkspaceLifetime = Literal["stage_call", "run", "manual"]
WorkspaceSurface = Literal["full_repo", "skills_only", "files_only", "custom"]


class WorkspaceHandle(BaseModel):
    """Materialized workspace reference. Auto-released at the configured lifetime."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    workspace_id: str
    candidate_id: str | None = None
    surface: WorkspaceSurface = "full_repo"
    lifetime: WorkspaceLifetime = "stage_call"
    receipt: CallReceipt
    """Materialize receipt; required for downstream effects in the same stage."""


class CandidateHandle(BaseModel):
    """Reference to a candidate in the run graph."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    id: str
    parent_id: str | None = None


__all__ = ["CandidateHandle", "WorkspaceHandle", "WorkspaceLifetime", "WorkspaceSurface"]
