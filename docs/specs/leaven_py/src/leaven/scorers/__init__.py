"""`lv.scorers.*` — premade scorer functions.

Each builder returns a `Scorer`-shaped callable
(`(RolloutResult, Case, Context) -> Awaitable[Score]`) that handles the
optional target internally — no `case.target[...]` indexing, no `assert`, no
`None`-unsafe access in user code. They cover the common labeled-task patterns
so the simple path needs zero scoring boilerplate.

Lineage: the `exact_match` / `includes` / `contains` vocabulary follows the
verifiers (Prime Intellect) and Inspect AI premade-scorer convention; the return
value is Leaven's `Score{value, feedback}`.

Governing spec: `docs/specs/leaven_python.md` — lv.scorers.
"""

from __future__ import annotations

from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from ..score import Scorer

__all__ = ["contains", "exact_match", "includes"]


def exact_match(*, field: str = "answer") -> Scorer:
    """A scorer that gives 1.0 when `str(run.output) == case.expect(field)`.

    Reads the labeled target via `Case.expect(field)` (clear error on an
    unlabeled case), so the scorer body the user would otherwise write is gone.
    """
    raise NotImplementedError("see leaven_python.md — lv.scorers.exact_match")


def includes(*, field: str = "answer") -> Scorer:
    """A scorer that gives 1.0 when the expected target value is a member of
    `run.output` (e.g. the answer appears in a produced collection)."""
    raise NotImplementedError("see leaven_python.md — lv.scorers.includes")


def contains(*, field: str = "answer") -> Scorer:
    """A scorer that gives 1.0 when `case.expect(field)` is a substring of
    `str(run.output)` (lenient containment match)."""
    raise NotImplementedError("see leaven_python.md — lv.scorers.contains")
