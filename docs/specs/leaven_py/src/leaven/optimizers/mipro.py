"""`lv.optimizers.mipro(...)` — MIPRO-v2 prompt optimization config (reserved scaffold)."""

from __future__ import annotations

from typing import Literal

from ..lm.config import LmConfig
from .config import OptimizerConfig


class Mipro(OptimizerConfig):
    """MIPROv2 optimizer config (reserved scaffold; pending Rust implementation)."""

    name: Literal["mipro"] = "mipro"
    num_candidates: int = 10
    num_trials: int = 20
    bootstrapping_lm: LmConfig | None = None
    instruction_lm: LmConfig | None = None


def mipro(
    *,
    num_candidates: int = 10,
    num_trials: int = 20,
    bootstrapping_lm: LmConfig | None = None,
    instruction_lm: LmConfig | None = None,
) -> Mipro:
    """MIPROv2 optimizer config builder (reserved scaffold)."""
    return Mipro(
        num_candidates=num_candidates,
        num_trials=num_trials,
        bootstrapping_lm=bootstrapping_lm,
        instruction_lm=instruction_lm,
    )


__all__ = ["Mipro", "mipro"]
