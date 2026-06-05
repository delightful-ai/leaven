"""`lv.dspy_acall(...)` — async-call a DSPy module under the current Leaven context.

DSPy's `Module.__call__` is sync; this wrapper makes it awaitable so it
composes with Leaven's async stage functions. The result is the DSPy
prediction object with a Leaven LM receipt attached as `.leaven_lm_receipt`.
"""

from collections.abc import Callable

from ..._receipts import CallReceipt
from ...json_value import JsonObject


class DspyPrediction:
    """A DSPy prediction wrapped with the Leaven LM receipt that produced it.

    Behaves like the underlying DSPy `Prediction`; access fields via
    attribute or `to_dict()`. The `leaven_lm_receipt` field carries the
    CallReceipt for downstream evidence binding.
    """

    def __init__(self, *, fields: JsonObject, leaven_lm_receipt: CallReceipt) -> None:
        self._fields = dict(fields)
        self.leaven_lm_receipt = leaven_lm_receipt

    def __getattr__(self, name: str) -> object:
        if name in self._fields:
            return self._fields[name]
        raise AttributeError(name)

    def to_dict(self) -> JsonObject:
        """The underlying prediction's data as a dict (DSPy convention)."""
        return dict(self._fields)


async def dspy_acall(module: Callable[..., object], **kwargs: object) -> DspyPrediction:
    """Async-invoke a DSPy module under the current Leaven context.

    `module` is any callable DSPy module (`dspy.Predict`, `dspy.ChainOfThought`,
    a custom `dspy.Module`, etc.). `kwargs` are passed through. Requires an
    enclosing `dspy_context(cx, ...)` block.
    """
    raise NotImplementedError("scaffold; see docs/specs/leaven_python.md")


__all__ = ["DspyPrediction", "dspy_acall"]
