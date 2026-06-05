"""`lv.x.dspy.LeavenDSPyLM` — drop-in `dspy.BaseLM` subclass.

Subclasses `dspy.BaseLM` and overrides `forward(prompt, messages, **kwargs)`
to lower into Leaven's `leaven-lm` neutral types and lift the response back
to OpenAI-chat-completion shape (what DSPy expects).

Usage:
    import dspy
    import leaven as lv

    dspy.configure(lm=lv.x.dspy.LeavenDSPyLM(model="claude-opus-4-7"))
    # all existing DSPy modules now route through Leaven

Per `docs/working-memory/leaven-py-research/2026-05-24-external-worker-prior-art.md`
§6 (DSPy-as-adapter-namespace pattern). Adapter is ~30 lines once leaven-lm
neutral types are wired.
"""

from collections.abc import Sequence
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    # dspy is an optional dep; keeping the base as object avoids requiring
    # dspy to be present for the import to resolve.
    DspyBaseLM: type[object] = object
else:
    try:
        from dspy import BaseLM as DspyBaseLM
    except ImportError:
        # Sentinel when dspy isn't installed; importing LeavenDSPyLM will
        # raise a clear error from `__init__`.
        DspyBaseLM = object


class LeavenDSPyLM(DspyBaseLM):  # type: ignore[misc, valid-type]
    """`dspy.BaseLM` subclass that routes through Leaven's LM seam.

    Constructor params mirror `dspy.LM`'s common kwargs (`model`,
    `model_type`, `cache`, etc.) so existing DSPy configurations slot in
    with minimal change. `cx` is required when used inside a Leaven stage
    (the context provides the wire); for standalone DSPy usage outside a
    stage, pass an explicit `lm_config` instead.
    """

    def __init__(
        self,
        *,
        model: str = "leaven-routed",
        model_type: str = "chat",
        cx: object | None = None,
        lm_config: object | None = None,
        cache: bool = True,
        **kwargs: object,
    ) -> None:
        """Construct a Leaven-backed DSPy LM.

        `cx` and `lm_config` are mutually exclusive. Inside a stage, pass
        `cx`. Outside a stage (e.g. in a script wired only for LM access),
        pass `lm_config=lv.lm.anthropic(...)` or similar.
        """
        raise NotImplementedError("scaffold; see docs/specs/leaven_python.md")

    def forward(
        self,
        prompt: str | None = None,
        messages: Sequence[dict[str, object]] | None = None,
        **kwargs: object,
    ) -> object:
        """DSPy LM contract: return an OpenAI-chat-shaped response."""
        raise NotImplementedError("scaffold; see docs/specs/leaven_python.md")

    async def aforward(
        self,
        prompt: str | None = None,
        messages: Sequence[dict[str, object]] | None = None,
        **kwargs: object,
    ) -> object:
        """Async variant of forward; preferred inside Leaven stages."""
        raise NotImplementedError("scaffold; see docs/specs/leaven_python.md")


__all__ = ["LeavenDSPyLM"]
