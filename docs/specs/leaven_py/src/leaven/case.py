"""Case record — the unit of evaluation work.

A Case carries input, target (hidden from runners/reflectors by default),
and metadata. Loaded via `cx.case.load(case_id, include=[...])`; the
include set determines which fields are projected (target-safe by default).
"""

from __future__ import annotations

from typing import Any

from pydantic import BaseModel, ConfigDict

from ._receipts import QueryReceipt


class Case(BaseModel):
    """A loaded case. Carries the read receipt for downstream evidence binding."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    id: str
    """Case identifier (paper-source-derived where applicable)."""

    input: dict[str, Any]
    """Inputs visible to runners. Always projected."""

    target: dict[str, Any] | None = None
    """Hidden answer(s). Visible only to evaluators/scorers/judges, never to
    runners or reflectors. None when the caller did not include target in the
    load projection."""

    metadata: dict[str, Any] | None = None
    """Source-side metadata (split, difficulty, etc.). Visible per projection."""

    target_ref: str | None = None
    """Opaque target reference for evidence binding without exposing target value."""

    receipt: QueryReceipt
    """Read receipt — pass into `EvidenceEnvelope.read_receipts` to bind."""


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
