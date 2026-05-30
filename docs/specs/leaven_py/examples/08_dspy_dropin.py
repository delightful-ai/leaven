"""Example 08 — DSPy drop-in via `lv.x.dspy`.

DSPy users live in Python and have specific LM-adapter expectations. The
integration is drop-in: `lv.x.dspy.LeavenDSPyLM` is a `dspy.BaseLM` subclass
that lowers into Leaven's neutral LM types, so existing DSPy code runs
unchanged through Leaven's LM seam. A DSPy `Program` can also be carried as a
Leaven artifact via `lv.x.dspy.artifact(program=...)` for GEPA-style evolution.

DSPy lives in the `lv.x.dspy` adapter namespace, not in core. The `dspy` import
is guarded, so importing `leaven` never requires DSPy. This example does not
import `dspy` itself; it only shows the composition shape.

Governing spec: `docs/specs/leaven_python.md` — DSPy.
"""

from __future__ import annotations

import leaven as lv


def main() -> None:
    try:
        # 1) Use Leaven's LM seam under DSPy. In real code:
        #       import dspy
        #       dspy.configure(lm=lv.x.dspy.LeavenDSPyLM(model="claude-opus-4-7"))
        #       program = dspy.ChainOfThought("question -> answer")
        lm = lv.x.dspy.LeavenDSPyLM(model="claude-opus-4-7")
        print(f"composed DSPy LM seam: {type(lm).__name__!r}")

        # 2) Carry a DSPy program as a Leaven artifact for evolution. The
        #    program's parameter state lowers into a Leaven artifact change-set.
        program = object()  # stand-in for a real `dspy.Program`
        artifact = lv.x.dspy.artifact(program=program)
        print(f"lowered DSPy program into artifact: {type(artifact).__name__!r}")
    except NotImplementedError as e:
        print(f"(expected) {e}")


if __name__ == "__main__":
    main()
