"""evolve — the entry point + typed result facade.

`await lv.evolve(artifact=, task=, stages=, optimizer=, runtime=).run()` returns
`Evolved[Artifact]`. The renamed verb (from `optimize`) reflects what Leaven
does: an artifact lineage with frontier admission, not parameter tuning.

`lv.optimize(...)` remains a deprecated alias through 0.2.x (spec lines 902-904);
it is NOT exported in the top-level `__all__`.

Governing spec: `docs/specs/leaven_python.md` — evolve.
"""

from __future__ import annotations

from collections.abc import Iterable, Mapping
from typing import TYPE_CHECKING, Any

from pydantic import BaseModel, ConfigDict

from .case import Case
from .propose import Proposal
from .score import Score

if TYPE_CHECKING:
    from .artifacts import Artifact
    from .optimizers import Optimizer
    from .runtime import Runtime
    from .stages import Stages
    from .task import Task

__all__ = [
    "Assessment",
    "Candidate",
    "Evolve",
    "Evolved",
    "ReplayResult",
    "RunSummary",
    "evolve",
]


class Candidate[A](BaseModel):
    """A candidate artifact in the run lineage."""

    model_config = ConfigDict(frozen=True, extra="forbid", arbitrary_types_allowed=True)

    id: str
    artifact: A
    proposal: Proposal | None = None
    scores: Mapping[str, float] = {}


class RunSummary(BaseModel):
    """Summary facts about a completed run."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    run_dir: str | None = None
    iterations: int = 0
    cost_usd: float = 0.0
    calls: int = 0
    replayable: bool = False


class Assessment(BaseModel):
    """A per-case assessment read off a run."""

    model_config = ConfigDict(frozen=True, extra="forbid", arbitrary_types_allowed=True)

    case: Case
    score: Score
    scorer_name: str
    replayable: bool = True


class ReplayResult(BaseModel):
    """The deterministic result of replaying one case."""

    model_config = ConfigDict(frozen=True, extra="forbid", arbitrary_types_allowed=True)

    case_id: str
    output: Any = None
    score: Score | None = None
    deterministic: bool = True


class Evolved[A]:
    """The typed result facade returned by `await evolve(...).run()`."""

    best: Candidate[A]
    frontier: list[Candidate[A]]
    summary: RunSummary

    def test_assessments(self) -> Iterable[Assessment]:
        """Per-case assessments over the test split."""
        raise NotImplementedError("see leaven_python.md — evolve")

    def assessment(self, case_id: str) -> Assessment:
        """The assessment for one case."""
        raise NotImplementedError("see leaven_python.md — evolve")

    async def replay(self, case_id: str) -> ReplayResult:
        """Deterministically replay one case's assessment."""
        raise NotImplementedError("see leaven_python.md — evolve")

    def lineage(self, candidate_id: str) -> Iterable[Candidate[A]]:
        """The ancestry chain for a candidate."""
        raise NotImplementedError("see leaven_python.md — evolve")


class Evolve:
    """The builder returned by `evolve(...)`; `await .run()` -> `Evolved[A]`."""

    async def run(self) -> Evolved[Any]:
        """Run the composed evolution to completion."""
        raise NotImplementedError("see leaven_python.md — evolve")


def evolve(
    *,
    artifact: Artifact,
    task: Task,
    stages: Stages,
    optimizer: Optimizer,
    runtime: Runtime,
) -> Evolve:
    """Compose an evolution: `artifact x task x stages x optimizer x runtime`.

    Returns an `Evolve` builder whose `.run()` awaits to `Evolved[A]`.
    """
    raise NotImplementedError("see leaven_python.md — evolve")


def optimize(*args: object, **kwargs: object) -> Evolve:
    """DEPRECATED alias for `evolve(...)`; emits a `DeprecationWarning`.

    Through 0.2.x only (spec lines 902-904). NOT in the top-level `__all__`.
    """
    raise NotImplementedError("deprecated alias; use lv.evolve(...). See leaven_python.md — evolve")
