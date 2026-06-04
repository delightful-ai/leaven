"""Rubric — the product scoring surface: a weighted vector of rewards.

A `Rubric` holds named reward functions and their weights. Each reward returns
a `RewardValue` (value + feedback + optional rich output) or a bare `float`.
The Rubric carries the VECTOR; the optimizer declares how it reduces (via its
`objective=`), so multi-objective optimization is never designed away by an
early scalar collapse. Per-reward metrics persist in evidence above `Score`.

`@lv.reward` is the authoring sugar; `Rubric([fn1, fn2])` takes the decorated
rewards directly. The engine lowers `Rollout + Rubric` into the same
assessment/evidence/receipt machinery a hand-written evaluator would author.
"""

from collections.abc import Awaitable, Callable
from typing import overload

from pydantic import BaseModel, ConfigDict

from .case import ScoringCaseView
from .contexts import RubricContext
from .output_record import OutputRecord


class RewardValue(BaseModel):
    """What a reward function returns when it wants more than a bare float.

    `value` is the reward scalar for this dimension (typically `[0, 1]`).
    `feedback` explains it (optimizer-visible). `output` optionally attaches a
    rich, visibility-labeled projection (text / json / blob / ...).
    """

    model_config = ConfigDict(frozen=True, extra="forbid")

    value: float
    feedback: str = ""
    output: OutputRecord | None = None


# A reward function body: `(output, case, cx) -> float | RewardValue`.
RewardFunc = Callable[
    [object, ScoringCaseView, RubricContext],
    Awaitable["float | RewardValue"],
]


class RegisteredReward(BaseModel):
    """A reward function bound with its weight and id. Built by `@lv.reward`."""

    model_config = ConfigDict(frozen=True, arbitrary_types_allowed=True, extra="forbid")

    id: str
    weight: float = 1.0
    func: RewardFunc
    """The wrapped async reward function. Engine-internal."""


@overload
def reward(func: RewardFunc, /) -> RegisteredReward: ...
@overload
def reward(
    *, weight: float = 1.0, id: str | None = None
) -> Callable[[RewardFunc], RegisteredReward]: ...
def reward(
    func: RewardFunc | None = None,
    *,
    weight: float = 1.0,
    id: str | None = None,
) -> RegisteredReward | Callable[[RewardFunc], RegisteredReward]:
    """Decorate an async `(output, case, cx) -> float | RewardValue` as a reward.

    Use bare (`@lv.reward`) for weight 1.0, or `@lv.reward(weight=0.3)` to set
    the weight. Pass the decorated rewards to `Rubric([...])`.
    """

    def wrap(f: RewardFunc) -> RegisteredReward:
        return RegisteredReward(
            id=id or f"{getattr(f, '__module__', 'leaven')}.{getattr(f, '__name__', 'reward')}",
            weight=weight,
            func=f,
        )

    return wrap(func) if func is not None else wrap


class Rubric(BaseModel):
    """The product scoring surface: a weighted vector of rewards.

    The engine runs each reward per `(candidate, case)`, keeps the per-reward
    vector + metrics in evidence, and reduces to the selection `Score` per the
    optimizer's `objective=`. `Rubric([r1, r2])` takes decorated rewards
    positionally.
    """

    model_config = ConfigDict(frozen=True, arbitrary_types_allowed=True, extra="forbid")

    rewards: list[RegisteredReward]

    def __init__(self, rewards: list[RegisteredReward]) -> None:
        super().__init__(rewards=rewards)


__all__ = ["RegisteredReward", "RewardFunc", "RewardValue", "Rubric", "reward"]
