"""Typed reflection records — `lv.adapters.reflective`.

The product-facing `ReflectiveBatch` a `ReflectFn` reads, plus its typed member
records. Heavy data (transcripts, env state) rides by `TraceRef` handle, never
inlined. These uphold build-once-pass-down (target-safe; `feedback` is the only
target-derived channel).

Governing spec: `docs/specs/leaven_python.md` — Reflect (batch shape).
"""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from typing import Any

from pydantic import BaseModel, ConfigDict

__all__ = [
    "Attachment",
    "ReflectiveBatch",
    "ReflectiveCase",
    "ReflectiveContext",
    "ReflectiveRun",
    "TraceRef",
]


class TraceRef(BaseModel):
    """A handle to heavy trajectory data (not inlined)."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    id: str


class Attachment(BaseModel):
    """A named attachment referencing heavy data by handle."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    name: str
    ref: TraceRef


class ReflectiveRun(BaseModel):
    """One scored run within a reflective case.

    Projects the Rust `ReflectiveRun` (see
    `docs/specs/gepa_reflection_evidence_visibility.md`): produced output, the
    scorer value, feedback (the only target-derived channel), plus the
    free-feedback failure signals `error` / `stop_condition` carried from the
    `RolloutResult`. Heavy data rides by `TraceRef`, never inlined.
    """

    model_config = ConfigDict(frozen=True, extra="forbid", arbitrary_types_allowed=True)

    output: Any
    score: float
    feedback: str = ""
    error: str | None = None
    stop_condition: str | None = None
    sessions: Sequence[str] = ()
    trajectory: TraceRef | None = None


class ReflectiveCase(BaseModel):
    """One case with its scored runs, target-safe.

    Projects the Rust `ReflectiveCase`: a target-safe `input`, an optional
    target-safe `expected`, and the per-attempt `runs`.
    """

    model_config = ConfigDict(frozen=True, extra="forbid", arbitrary_types_allowed=True)

    case_id: str | None = None
    input: Mapping[str, Any]
    expected: Mapping[str, Any] | None = None
    runs: Sequence[ReflectiveRun] = ()


class ReflectiveBatch(BaseModel):
    """The product-facing pre-built batch a `ReflectFn` reads."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    cases: Sequence[ReflectiveCase] = ()


class ReflectiveContext(BaseModel):
    """Input to the `reflective_dataset=` hook (GEPA policy, engine-side)."""

    model_config = ConfigDict(frozen=True, extra="forbid", arbitrary_types_allowed=True)

    cases: Sequence[ReflectiveCase] = ()
