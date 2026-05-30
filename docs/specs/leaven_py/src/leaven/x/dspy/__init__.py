"""`lv.x.dspy.*` — the DSPy adapter namespace.

`LeavenDSPyLM` is a `dspy.BaseLM` subclass (import-guarded) that lowers into
Leaven's neutral LM types. `artifact(program=...)` lowers a DSPy program's
parameter state into a Leaven-native artifact change-set.

Governing spec: `docs/specs/leaven_python.md` — DSPy.
"""

from __future__ import annotations

from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from ...artifacts import Artifact

__all__ = ["LeavenDSPyLM", "artifact"]


class LeavenDSPyLM:
    """A `dspy.BaseLM` subclass lowering into Leaven neutral LM types.

    The `dspy` import is guarded so importing `leaven` does not require DSPy.
    """

    def __init__(self, *, model: str, **kwargs: object) -> None:
        raise NotImplementedError("see leaven_python.md — DSPy")


def artifact(*, program: object, **kwargs: object) -> Artifact:
    """Lower a DSPy program's parameter state into a Leaven artifact."""
    raise NotImplementedError("see leaven_python.md — DSPy")
