"""Example 03 — the canonical minimal sketch.

The smallest meaningful Leaven program: optimize a prompt for an arithmetic
QA task with GEPA, exact-match scored, against a local runtime with a mock LM.

The whole program is `seed x Environment(task, rollout, rubric) x optimizer x
runtime`. The rollout is target-FREE (`InputCaseView` has no `.target`); the
rubric reads the target (`ScoringCaseView`). Everything else is composition of
typed configs — trivial problems write tiny.
"""

from __future__ import annotations

import asyncio
from pathlib import Path

import leaven as lv

HERE = Path(__file__).parent
FIXTURE = HERE / "fixtures" / "arithmetic.jsonl"


# ----- rollout: how the current artifact runs on one case -------------------
# `Rollout.fn` wraps a plain async runner. The rollout `cx` is TARGET-FREE: it
# sees `case.input`, can drive `cx.lm` / `cx.agent` / `cx.sandbox`, but `case`
# is an `InputCaseView` — it has no `.target` attribute at all (structural).
@lv.runner
async def run(prompt: lv.PromptArtifact, case: lv.InputCaseView, cx: lv.RolloutContext) -> str:
    reply = await cx.lm.complete(prompt=prompt.template.format(**case.input), max_tokens=64)
    return reply.text.strip()


# ----- rubric: how the rollout's output scores ------------------------------
# `@lv.reward` is the authoring sugar; `Rubric([...])` collects rewards. The
# rubric `cx` is scorer-role: `case` is a `ScoringCaseView`, so `case.target`
# is readable here (gated + receipted under the hood).
@lv.reward
async def exact(output: str, case: lv.ScoringCaseView, cx: lv.RubricContext) -> float:
    return 1.0 if output == (case.target or {})["answer"] else 0.0


# ----- composition ----------------------------------------------------------
async def amain() -> None:
    result = await lv.optimize(
        seed=lv.PromptArtifact(template="Answer: {question}\nA:"),
        environment=lv.Environment(
            task=lv.Task(cases=lv.cases.from_jsonl(str(FIXTURE), limit=8).cases),
            rollout=lv.Rollout.fn(run),
            rubric=lv.Rubric([exact]),
        ),
        optimizer=lv.optimizers.gepa(population_size=8),
        runtime=lv.runtime.local(budget=lv.budget(usd=20)),
    ).run()

    # `.run()` raises NotImplementedError in the scaffold; once the engine is
    # wired, `result.best.artifact` is a fully-typed `PromptArtifact`.
    print(result.best.artifact.template)


if __name__ == "__main__":
    try:
        asyncio.run(amain())
    except NotImplementedError as e:
        print(f"(expected) {e}")
