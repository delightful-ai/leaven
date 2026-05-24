"""Optimizer config base."""

from __future__ import annotations

from pydantic import BaseModel, ConfigDict


class OptimizerConfig(BaseModel):
    """Common optimizer config. Optimizer-specific subclasses add fields."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    name: str
    """Optimizer name (e.g. 'gepa', 'mipro', 'seed_best')."""


__all__ = ["OptimizerConfig"]
