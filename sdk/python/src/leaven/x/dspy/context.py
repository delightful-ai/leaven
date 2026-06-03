"""`lv.dspy_context(...)` / `lv.dspy_call_context(...)` — visibility scoping for DSPy calls.

These context managers scope DSPy LM calls inside an evaluator with explicit
data-class declarations. Without them, DSPy calls would have to declare
input/forbidden classes individually; the context propagates a default that
all calls inherit.

Re-exported at top-level as `lv.dspy_context` and `lv.dspy_call_context`
because they appear inline in evaluator bodies (the locked spec example
uses both).
"""

from collections.abc import AsyncIterator, Iterator, Sequence
from contextlib import asynccontextmanager, contextmanager
from typing import Any


@contextmanager
def dspy_context(
    cx: Any,
    *,
    model_role: str | None = None,
    strict: bool = True,
) -> Iterator[None]:
    """Bind a Leaven context for use by DSPy calls inside the block.

    Inside the block, any `LeavenDSPyLM` instance (or any DSPy module
    configured to use one) routes LM calls through `cx.lm.*`. `strict=True`
    means DSPy calls outside this context raise rather than fall through.
    """
    raise NotImplementedError("scaffold; see docs/specs/leaven_python.md")
    yield  # type: ignore[unreachable] — body is scaffold; yield satisfies generator protocol


@asynccontextmanager
async def dspy_call_context(
    *,
    input_classes: Sequence[str] | None = None,
    forbidden_input_classes: Sequence[str] | None = None,
) -> AsyncIterator[None]:
    """Declare data-class scoping for the DSPy calls inside the block.

    Sets defaults that `LeavenDSPyLM.forward(...)` reads when the user
    hasn't passed `input_classes` / `forbidden_input_classes` explicitly.
    Nests inside `dspy_context(cx, ...)`.
    """
    raise NotImplementedError("scaffold; see docs/specs/leaven_python.md")
    yield  # type: ignore[unreachable] — body is scaffold; yield satisfies async-generator protocol


__all__ = ["dspy_call_context", "dspy_context"]
