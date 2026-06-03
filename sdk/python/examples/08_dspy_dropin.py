"""Example 08 — DSPy drop-in: existing DSPy modules run unmodified through Leaven.

The adapter shape is one line: `dspy.configure(lm=lv.x.dspy.LeavenDSPyLM(...))`.
Existing `dspy.Predict`, `dspy.ChainOfThought`, custom `dspy.Module`s all
route through Leaven's LM seam with capability tokens, receipts, and
data-class propagation preserved.

Inside an evaluator, `dspy_context(cx, ...)` scopes DSPy calls to the
current stage's context; `dspy_call_context(...)` declares data-class
defaults; `dspy_acall(...)` async-invokes a DSPy module and returns a
prediction with `.leaven_lm_receipt` attached for evidence binding.

This example is the smallest meaningful DSPy integration: configure +
predict, no Leaven optimization at all. Use this when you want DSPy
modules to benefit from Leaven's wire safety without restructuring.
"""

import leaven as lv

# `dspy` is an optional dep. Install with `uv add dspy-ai` or
# `pip install 'leaven[dspy]'` to run this example end-to-end.
try:
    import dspy
except ImportError:
    dspy = None  # type: ignore[assignment]


def main() -> None:
    if dspy is None:
        print("dspy not installed; install with: uv add dspy-ai")
        return

    # Configure DSPy to route all LM calls through Leaven. Outside a Leaven
    # stage, pass an explicit `lm_config=`; inside a stage, pass `cx=`.
    dspy.configure(
        lm=lv.x.dspy.LeavenDSPyLM(
            model="claude-opus-4-7",
            lm_config=lv.lm.anthropic(model="claude-opus-4-7"),
        ),
    )

    # Now any DSPy module runs unmodified.
    qa = dspy.Predict("question -> answer")
    print("dspy module:", qa)

    # The actual call raises NotImplementedError until the LM seam wires:
    # result = qa(question="What is 2 + 2?")
    # print(result.answer)


if __name__ == "__main__":
    main()
