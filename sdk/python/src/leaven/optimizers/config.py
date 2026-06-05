"""Optimizer config base."""

from pydantic import BaseModel, ConfigDict


class OptimizerConfig(BaseModel):
    """Common optimizer config. Optimizer-specific subclasses add fields."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    name: str
    """Optimizer name (e.g. 'gepa', 'seed_best')."""


__all__ = ["OptimizerConfig"]
