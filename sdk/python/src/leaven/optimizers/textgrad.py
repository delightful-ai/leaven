"""`lv.optimizers.textgrad(...)` — TextGrad config (reserved scaffold)."""

from typing import Literal

from .config import OptimizerConfig


class TextGrad(OptimizerConfig):
    """TextGrad optimizer config (reserved scaffold; pending Rust implementation)."""

    name: Literal["textgrad"] = "textgrad"
    learning_rate: float = 0.1
    max_iterations: int = 50


def textgrad(*, learning_rate: float = 0.1, max_iterations: int = 50) -> TextGrad:
    """TextGrad optimizer config builder (reserved scaffold)."""
    return TextGrad(learning_rate=learning_rate, max_iterations=max_iterations)


__all__ = ["TextGrad", "textgrad"]
