"""`lv.frontier.*` — frontier policy configs passed to optimizer builders.

The frontier is the set of candidates the optimizer keeps as the working
population. Different policies have different admission/eviction semantics.
"""

from __future__ import annotations

from typing import Literal

from pydantic import BaseModel, ConfigDict


class FrontierConfig(BaseModel):
    """Common frontier config."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    kind: str


class TopK(FrontierConfig):
    """Top-K frontier — keep the K best by validation score; evict the weakest."""

    kind: Literal["top_k"] = "top_k"
    k: int


class Pareto(FrontierConfig):
    """Pareto frontier — keep non-dominated candidates across multiple metrics."""

    kind: Literal["pareto"] = "pareto"
    metrics: list[str]


def top_k(k: int) -> TopK:
    """Top-K frontier policy.

    The K best candidates by validation score are retained; weaker candidates
    are evicted when better ones admit. K=3 is the EvoSkill paper default.
    """
    return TopK(k=k)


def pareto(*, metrics: list[str]) -> Pareto:
    """Pareto frontier policy.

    Non-dominated candidates across the named metrics are retained. The
    optimizer must support multi-metric admission (not all do).
    """
    return Pareto(metrics=metrics)


__all__ = ["FrontierConfig", "Pareto", "TopK", "pareto", "top_k"]
