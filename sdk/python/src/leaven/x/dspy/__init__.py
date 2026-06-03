"""`lv.x.dspy.*` — DSPy adapter namespace.

The DSPy drop-in is the proof case for the adapter namespace pattern. Users
configure DSPy with `dspy.configure(lm=lv.x.dspy.LeavenDSPyLM(...))` and
their existing DSPy modules run unchanged through Leaven's LM seam.

The context helpers (`dspy_context`, `dspy_acall`, `dspy_call_context`)
provide visibility/data-class scoping for DSPy calls inside an evaluator.
"""

from .context import dspy_call_context, dspy_context
from .invoke import dspy_acall
from .lm import LeavenDSPyLM

__all__ = ["LeavenDSPyLM", "dspy_acall", "dspy_call_context", "dspy_context"]
