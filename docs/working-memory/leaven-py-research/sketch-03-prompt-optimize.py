"""Sketch 03 — canonical minimal, on the REDESIGNED surface.

Redesign sketch (2026-06-01). Embodies the agreed shape:

    seed × Environment(task, rollout, rubric) × optimizer × runtime

with the `cx` role-split visible in real code. NOT yet importable —
`lv.Environment`, `lv.Rubric`, `@lv.reward`, `lv.Rollout.fn` are not in the
scaffold package yet (wiring them is the next step). Compare against the
current surface in `examples/03_prompt_optimize.py`.

The whole program: optimize a prompt for arithmetic QA with GEPA, exact-match
scored, against a local runtime.
"""

from __future__ import annotations

import asyncio

import leaven as lv


# ----- rollout: how the current artifact runs on one case -------------------
# A `Rollout.fn` wraps a plain async function (the old `@lv.runner` body).
# rollout-`cx` is TARGET-FREE: it sees `case.input`, can drive `cx.lm` /
# `cx.agent` / `cx.sandbox` / `cx.workspace`, but it CANNOT read `case.target`
# and CANNOT mutate the graph. The function's return value IS `output`.
async def run(prompt: lv.PromptArtifact, case: lv.InputCaseView, cx: lv.RolloutContext) -> str:
    reply = await cx.lm.complete(prompt=prompt.template.format(**case.input))
    return reply.text.strip()  # `case` has no `.target` attribute at all — structural


# ----- rubric: how the rollout's output scores ------------------------------
# `@lv.reward` is the authoring sugar; a `Rubric` takes the decorated rewards
# directly. rubric-`cx` is SCORER-role: `output` is the rollout result,
# `case.target` IS readable here, and `cx` is available for judge-style rewards
# (`cx.lm`) or inspecting `cx.rollout_workspace`. Still no graph mutation.
@lv.reward
async def exact(output: str, case: lv.ScoringCaseView, cx: lv.RubricContext) -> float:
    return 1.0 if output == case.target["answer"] else 0.0  # ScoringCaseView → target readable


# ----- composition ----------------------------------------------------------
async def amain() -> None:
    result = await lv.optimize(
        seed=lv.PromptArtifact(template="Answer: {question}\nA:"),
        environment=lv.Environment(
            task=lv.Task(cases=lv.cases.from_jsonl("arithmetic.jsonl", limit=8)),
            rollout=lv.Rollout.fn(run),
            rubric=lv.Rubric([exact]),
        ),
        optimizer=lv.optimizers.gepa(population_size=8),
        runtime=lv.runtime.local(budget=lv.budget(usd=20)),
    ).run()

    print(result.best.artifact.template)


if __name__ == "__main__":
    try:
        asyncio.run(amain())
    except NotImplementedError as e:
        print(f"(expected) {e}")
