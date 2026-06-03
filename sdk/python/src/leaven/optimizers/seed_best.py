"""`lv.optimizers.seed_best()` — trivial baseline: just evaluate the seed."""

from typing import Literal

from .config import OptimizerConfig


class SeedBest(OptimizerConfig):
    """No-op optimizer: just runs the seed against the cases and reports.

    Useful as a baseline and as the default when no optimizer is specified.
    """

    name: Literal["seed_best"] = "seed_best"


def seed_best() -> SeedBest:
    """Trivial baseline optimizer; useful for smoke tests."""
    return SeedBest()


__all__ = ["SeedBest", "seed_best"]
