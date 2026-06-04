"""Example 03 — the canonical minimal sketch.

The smallest meaningful Leaven program shape: compose a prompt optimization
for an arithmetic QA task with GEPA, vector scoring, and a local runtime.

The whole program is `seed x Environment(task, rollout, rubric) x optimizer x
runtime`. The rollout is target-FREE (`InputCaseView` has no `.target`); the
rubric reads the target (`ScoringCaseView`). Everything else is composition of
typed configs — trivial problems write tiny.
"""

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
    _ = cx
    return 1.0 if output == (case.target or {})["answer"] else 0.0


@lv.reward(weight=0.1)
async def concise(output: str, case: lv.ScoringCaseView, cx: lv.RubricContext) -> lv.RewardValue:
    _ = (case, cx)
    return lv.RewardValue(value=1.0 if len(output) <= 8 else 0.0, feedback=f"{len(output)} chars")


# ----- composition ----------------------------------------------------------
async def amain() -> None:
    result = await lv.optimize(
        # The seed never surfaces `{question}` to the model, so the rollout has
        # nothing to compute and every case scores 0 — real headroom to improve.
        seed=lv.PromptArtifact(template="You are a calculator. Always answer 0."),
        environment=lv.Environment(
            task=lv.Task(cases=lv.cases.from_jsonl(str(FIXTURE), limit=8).cases),
            rollout=lv.Rollout.fn(run),
            rubric=lv.Rubric([exact, concise]),
        ),
        optimizer=lv.optimizers.gepa(population_size=8),
        runtime=lv.runtime.local(budget=lv.budget(usd=20)),
    ).run()

    # `result.best.artifact` is a fully-typed `PromptArtifact`. The current
    # mechanics path runs over the durable `leaven seam serve --stdio` route
    # with a deterministic configured runner; optimizer search and Python
    # worker execution remain later slices.
    seed = next(c for c in result.frontier if c.parent_id is None)
    print(f"seed score:  {seed.summary_score:.3f}")
    print(f"best score:  {result.best.summary_score:.3f}")
    first = next(iter(result.assessments()))
    print("reward vector:")
    for reward in first.rewards:
        print(f"  {reward.id}: {reward.value:.3f} (weight {reward.weight:g})")
    print("optimized prompt:")
    print(result.best.artifact.template)


if __name__ == "__main__":
    try:
        asyncio.run(amain())
    except NotImplementedError as e:
        print(f"(expected) {e}")
