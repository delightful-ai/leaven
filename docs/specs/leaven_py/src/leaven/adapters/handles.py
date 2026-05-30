"""Advanced-authoring annotation nouns — `lv.adapters` handle/case types.

These six nouns are spec-promised ("importable from a ring") but were previously
named-only. They are typed stub annotation types for advanced authoring; they
are NOT product nouns (forbidden from top-level `lv.__all__`). Ordinary
runner/scorer code never imports them.

- `RunCase` / `ScoreCase`: the per-stage case projections the engine hands a
  rollout vs. a scorer (a rollout sees no target; a scorer sees the target).
- `CandidateHandle`: an opaque handle to a candidate artifact for advanced
  evaluator effects (`materialize_candidate`).
- `WorkspaceHandle` / `WorkspaceLifetime` / `WorkspaceSurface`: the workspace
  allocation/lifetime/projection vocabulary for advanced authoring.

Governing spec: `docs/specs/leaven_python.md` — Public API discipline (rings).
"""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from enum import StrEnum
from typing import Any

from pydantic import BaseModel, ConfigDict

__all__ = [
    "CandidateHandle",
    "RunCase",
    "ScoreCase",
    "WorkspaceHandle",
    "WorkspaceLifetime",
    "WorkspaceSurface",
]


class RunCase(BaseModel):
    """The target-free case projection a rollout receives."""

    model_config = ConfigDict(frozen=True, extra="forbid", arbitrary_types_allowed=True)

    id: str
    input: Mapping[str, Any]


class ScoreCase(BaseModel):
    """The case projection a scorer receives (adds the target)."""

    model_config = ConfigDict(frozen=True, extra="forbid", arbitrary_types_allowed=True)

    id: str
    input: Mapping[str, Any]
    target: Mapping[str, Any] | None = None


class CandidateHandle(BaseModel):
    """An opaque handle to a candidate artifact for advanced evaluator effects."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    id: str


class WorkspaceLifetime(StrEnum):
    """How long an allocated workspace lives."""

    rollout = "rollout"
    case = "case"
    run = "run"


class WorkspaceSurface(BaseModel):
    """The projected file surface of a workspace (mutable + read-only paths)."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    mutable: Sequence[str] = ()
    read_only: Sequence[str] = ()


class WorkspaceHandle(BaseModel):
    """An opaque handle to an allocated workspace, with lifetime + surface."""

    model_config = ConfigDict(frozen=True, extra="forbid", arbitrary_types_allowed=True)

    id: str
    lifetime: WorkspaceLifetime = WorkspaceLifetime.rollout
    surface: WorkspaceSurface | None = None
