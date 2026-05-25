"""`lv.optimizers.gepa(...)` — GEPA (reflective two-phase optimization) config."""

from __future__ import annotations

from typing import Literal

from ..frontier import FrontierConfig
from ..lm.config import LmConfig
from .config import OptimizerConfig

ParentSelector = Literal["round_robin", "best", "weighted", "pareto"]


class Gepa(OptimizerConfig):
    """GEPA optimizer config."""

    name: Literal["gepa"] = "gepa"

    population_size: int = 10
    """Working population size; engine spawns candidates up to this cap."""

    frontier: FrontierConfig | None = None
    """Frontier policy; `lv.frontier.top_k(3)` is the EvoSkill default."""

    parent_selector: ParentSelector = "round_robin"
    """Strategy for picking which parent to reflect against next."""

    reflection_lm: LmConfig | None = None
    """LM for reflection calls. Inherits runtime LM if omitted."""

    minibatch_size: int = 4
    """Cases per reflection minibatch."""

    max_iterations: int | None = None
    """Hard cap on optimization iterations. None = budget-bounded."""


def gepa(
    *,
    population_size: int = 10,
    frontier: FrontierConfig | None = None,
    parent_selector: ParentSelector = "round_robin",
    reflection_lm: LmConfig | None = None,
    minibatch_size: int = 4,
    max_iterations: int | None = None,
) -> Gepa:
    """GEPA optimizer config builder.

    GEPA (Genetic Evolutionary Prompt Adaptation) is the reflective two-phase
    optimizer. Stage roles: reflector (understands), proposer (decides).
    Defaults match the paper's EvoSkill configuration.
    """
    return Gepa(
        population_size=population_size,
        frontier=frontier,
        parent_selector=parent_selector,
        reflection_lm=reflection_lm,
        minibatch_size=minibatch_size,
        max_iterations=max_iterations,
    )


__all__ = ["Gepa", "ParentSelector", "gepa"]
