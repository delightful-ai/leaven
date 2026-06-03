"""Result types — `Optimized[A]`, `Candidate`, `RunSummary`, `ReplayResult`.

What `await lv.optimize(...).run()` returns and what `lv.runs.open(...)`
reads back.
"""

from __future__ import annotations

from collections.abc import Iterable
from typing import Literal

from pydantic import BaseModel, ConfigDict

from .assessment import Assessment, Replayability
from .run_status import RunCostStatus, RunUsageStatus, UnsupportedRunFact


class Candidate[A](BaseModel):
    """A candidate in the run graph."""

    model_config = ConfigDict(frozen=True, arbitrary_types_allowed=True, extra="forbid")

    id: str
    artifact: A
    parent_id: str | None = None
    summary_score: float | None = None
    """Aggregate validation-set score; None if not yet evaluated."""

    def summary(self) -> str:
        """One-line summary of this candidate."""
        raise NotImplementedError("scaffold; see docs/specs/leaven_python.md")


class RunSummary(BaseModel):
    """Aggregate facts about a completed run."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    run_id: str
    started_at: str
    """ISO 8601 timestamp."""
    completed_at: str | None
    iterations: int
    candidates_evaluated: int
    total_cost_usd: float | None
    cost_status: RunCostStatus = "known"
    total_calls: int
    total_lm_tokens: int | None
    usage_status: RunUsageStatus = "known"
    unsupported: tuple[UnsupportedRunFact, ...] = ()
    replayability: Replayability
    """Roll-up across assessments; `non_replayable` if any one assessment is."""


Split = Literal["train", "val", "test"]


class ReplayResult(BaseModel):
    """What a replay produces; same shape as the original assessment."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    case_id: str
    candidate_id: str
    score: float
    matches_original: bool
    """True iff the replay's score matches the original within tolerance."""


class Optimized[A](BaseModel):
    """Result of `await lv.optimize(...).run()`. Typed by artifact."""

    model_config = ConfigDict(frozen=True, arbitrary_types_allowed=True, extra="forbid")

    run_id: str
    """Engine-minted run id; pass to `lv.runs.open(path)` to re-open."""
    best: Candidate[A]
    frontier: list[Candidate[A]]
    summary: RunSummary

    def assessment(self, case_id: str, *, candidate_id: str | None = None) -> Assessment:
        """Look up one assessment by case id (and optionally candidate id)."""
        raise NotImplementedError("scaffold; see docs/specs/leaven_python.md")

    def test_assessments(self) -> Iterable[Assessment]:
        """Iterate assessments from the held-out test split."""
        raise NotImplementedError("scaffold; see docs/specs/leaven_python.md")

    def assessments(
        self,
        *,
        split: Split | None = None,
        candidate_id: str | None = None,
    ) -> Iterable[Assessment]:
        """Filter assessments by split and/or candidate."""
        raise NotImplementedError("scaffold; see docs/specs/leaven_python.md")

    def lineage(self, candidate_id: str) -> Iterable[Candidate[A]]:
        """Walk ancestor candidates from `candidate_id` back to a seed."""
        raise NotImplementedError("scaffold; see docs/specs/leaven_python.md")

    async def replay(
        self,
        case_id: str,
        *,
        candidate_id: str | None = None,
    ) -> ReplayResult:
        """Replay one assessment deterministically; verify the receipt holds."""
        raise NotImplementedError("scaffold; see docs/specs/leaven_python.md")


__all__ = ["Candidate", "Optimized", "ReplayResult", "RunSummary", "Split"]
