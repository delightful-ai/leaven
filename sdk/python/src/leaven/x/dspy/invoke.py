"""`lv.dspy_acall(...)` — async-call a DSPy module under the current Leaven context.

DSPy's `Module.__call__` is sync; this wrapper makes it awaitable so it
composes with Leaven's async stage functions. The result is the DSPy
prediction object with a Leaven LM receipt attached as `.leaven_lm_receipt`.
"""

from __future__ import annotations

from typing import Any

from ..._receipts import CallReceipt


class DspyPrediction:
    """A DSPy prediction wrapped with the Leaven LM receipt that produced it.

    Behaves like the underlying DSPy `Prediction`; access fields via
    attribute or `to_dict()`. The `leaven_lm_receipt` field carries the
    CallReceipt for downstream evidence binding.
    """

    leaven_lm_receipt: CallReceipt

    def __getattr__(self, name: str) -> Any:
        raise NotImplementedError("scaffold; see docs/specs/leaven_python.md")

    def to_dict(self) -> dict[str, Any]:
        """The underlying prediction's data as a dict (DSPy convention)."""
        raise NotImplementedError("scaffold; see docs/specs/leaven_python.md")


async def dspy_acall(module: Any, **kwargs: Any) -> DspyPrediction:
    """Async-invoke a DSPy module under the current Leaven context.

    `module` is any callable DSPy module (`dspy.Predict`, `dspy.ChainOfThought`,
    a custom `dspy.Module`, etc.). `kwargs` are passed through. Requires an
    enclosing `dspy_context(cx, ...)` block.
    """
    raise NotImplementedError("scaffold; see docs/specs/leaven_python.md")


__all__ = ["DspyPrediction", "dspy_acall"]
