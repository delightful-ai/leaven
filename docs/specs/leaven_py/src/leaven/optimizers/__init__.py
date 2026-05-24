"""Optimizer registry — `lv.optimizers.gepa(...)`, `lv.optimizers.mipro(...)`, etc.

Each builder returns a typed config the engine instantiates the corresponding
Rust optimizer with. New optimizers require a new Rust crate; Python users
configure existing ones.
"""

from __future__ import annotations

from .config import OptimizerConfig
from .gepa import Gepa, gepa
from .mipro import Mipro, mipro
from .seed_best import SeedBest, seed_best
from .textgrad import TextGrad, textgrad

__all__ = [
    "Gepa",
    "Mipro",
    "OptimizerConfig",
    "SeedBest",
    "TextGrad",
    "gepa",
    "mipro",
    "seed_best",
    "textgrad",
]
