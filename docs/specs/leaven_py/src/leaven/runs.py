"""runs — `lv.runs.open(path)` to inspect a completed run.

`Run` mirrors the `Evolved` surface (`.best/.frontier/.summary/
.test_assessments/.lineage/.replay`) but is a distinct read-only class: it
opens a completed run directory for inspection rather than driving a live run.

Governing spec: `docs/specs/leaven_python.md` — evolve (inspection).
"""

from __future__ import annotations

from collections.abc import Iterable
from typing import Any

from .evolve import Assessment, Candidate, ReplayResult, RunSummary

__all__ = ["Run", "open"]


class Run:
    """A read-only view of a completed run directory."""

    best: Candidate[Any]
    frontier: list[Candidate[Any]]
    summary: RunSummary

    def test_assessments(self) -> Iterable[Assessment]:
        """Per-case assessments over the test split."""
        raise NotImplementedError("see leaven_python.md — runs.open")

    def assessment(self, case_id: str) -> Assessment:
        """The assessment for one case."""
        raise NotImplementedError("see leaven_python.md — runs.open")

    def lineage(self, candidate_id: str) -> Iterable[Candidate[Any]]:
        """The ancestry chain for a candidate."""
        raise NotImplementedError("see leaven_python.md — runs.open")

    async def replay(self, case_id: str) -> ReplayResult:
        """Deterministically replay one case's assessment."""
        raise NotImplementedError("see leaven_python.md — runs.open")


def open(path: str) -> Run:
    """Open a completed run directory for inspection.

    Spec: `lv.runs.open(".leaven/runs/2026-05-25-codex-ctf")`.
    """
    raise NotImplementedError("see leaven_python.md — runs.open")
