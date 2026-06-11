"""`lv.optimizers.gepa(...)` — GEPA (reflective two-phase optimization) config."""

from typing import Literal

from ..frontier import FrontierConfig
from ..lm.config import LmConfig
from ..stages import Propose, Reflect
from .config import OptimizerConfig

ParentSelector = Literal["round_robin", "best", "weighted", "pareto"]
Objective = Literal["instance", "objective", "hybrid", "cartesian"]
"""How the rubric's reward vector drives selection (GEPA `frontier_type`).

Named rewards are the objective dimensions; `instance` = per-case Pareto
(default), `objective` = per-reward-dimension Pareto, `hybrid`/`cartesian` per
GEPA. Reward weights feed the aggregate scalar used for tie-breaks.
"""


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

    objective: Objective = "instance"
    """Pareto frontier axis over the rubric's reward vector (GEPA frontier_type)."""

    reflect: Reflect | None = None
    """Reflection stage override; None uses GEPA's built-in reflector."""

    propose: Propose | None = None
    """Proposal stage override; None uses GEPA's built-in proposer."""


def gepa(
    *,
    population_size: int = 10,
    frontier: FrontierConfig | None = None,
    parent_selector: ParentSelector = "round_robin",
    reflection_lm: LmConfig | None = None,
    minibatch_size: int = 4,
    max_iterations: int | None = None,
    objective: Objective = "instance",
    reflect: Reflect | None = None,
    propose: Propose | None = None,
) -> Gepa:
    """GEPA optimizer config builder.

    GEPA (Genetic Evolutionary Prompt Adaptation) is the reflective two-phase
    optimizer. Stage roles: reflector (understands), proposer (decides).

    V1 `lv.optimize(...).run()` honors `population_size` (the candidate-pool cap,
    `>= 2`), `minibatch_size` (the train screening minibatch), `objective`
    (`"instance"` only; the host refuses other objectives), and an `lm`
    reflection model (from `reflection_lm` or the runtime LM). The remaining
    knobs (`frontier`, `parent_selector`, `max_iterations`, `reflect`,
    `propose`) have no `leaven/optimize.run` route in V1 and are refused at
    lowering rather than silently ignored.
    """
    return Gepa(
        population_size=population_size,
        frontier=frontier,
        parent_selector=parent_selector,
        reflection_lm=reflection_lm,
        minibatch_size=minibatch_size,
        max_iterations=max_iterations,
        objective=objective,
        reflect=reflect,
        propose=propose,
    )


__all__ = ["Gepa", "Objective", "ParentSelector", "gepa"]
