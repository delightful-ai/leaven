"""Case records — the public task instance and execution projection.

Users author `Case` values in `lv.Task(cases=[...])`; stages also receive a
case-shaped projection selected by the engine for the current role. The engine
tracks receipts and redactions internally; ordinary user code should not thread
case read receipts by hand.
"""

from __future__ import annotations

from typing import Any

from pydantic import BaseModel, ConfigDict, Field

from .sandbox.config import SandboxConfig
from .setup import SetupScript


class Case(BaseModel):
    """One benchmark/task case."""

    model_config = ConfigDict(frozen=True, arbitrary_types_allowed=True, extra="forbid")

    id: str
    """Case identifier (paper-source-derived where applicable)."""

    input: dict[str, Any]
    """Inputs visible to runners."""

    target: dict[str, Any] | None = None
    """Hidden answer(s) / rubric. Projected only to target-authorized roles."""

    metadata: dict[str, Any] = Field(default_factory=dict)
    """Source-side metadata such as split, difficulty, and provenance."""

    files: dict[str, str] = Field(default_factory=dict)
    """Case files/assets materialized by rollout layouts."""

    setup: SetupScript | None = None
    """Optional setup action run after files are materialized."""

    sandbox: SandboxConfig | None = None
    """Optional per-case sandbox override."""

    split: str | None = None
    """Optional split tag used when a `Task` is lowered into train/val/test sets."""


class CaseSet(BaseModel):
    """A named set of cases — train / validation / test."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    name: str
    """Set name (typically 'train', 'val', 'test')."""

    cases: list[Case]


class CaseSplits(BaseModel):
    """A train/val/test bundle as loaded from a benchmark."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    train: CaseSet
    val: CaseSet | None = None
    test: CaseSet | None = None


__all__ = ["Case", "CaseSet", "CaseSplits"]
