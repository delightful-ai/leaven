"""Optimizer registry — `lv.optimizers.gepa(...)`, `lv.optimizers.seed_best()`.

Each builder returns a typed config the engine instantiates the corresponding
Rust optimizer with. New optimizers require a new Rust crate; Python users
configure existing ones.
"""

from .config import OptimizerConfig
from .gepa import Gepa, gepa
from .seed_best import SeedBest, seed_best

__all__ = [
    "Gepa",
    "OptimizerConfig",
    "SeedBest",
    "gepa",
    "seed_best",
]
