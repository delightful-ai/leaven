"""Example 03 — the canonical minimal sketch.

The smallest meaningful Leaven program: optimize a prompt for an
arithmetic QA task with GEPA against a local environment with a mock LM.

~25 lines of user code; everything else is composition of typed configs.
This is the 200-line-target's lower bound: trivial problems write tiny.
"""

from __future__ import annotations

import asyncio
from pathlib import Path

import leaven as lv

HERE = Path(__file__).parent
FIXTURE = HERE / "fixtures" / "arithmetic.jsonl"


# ---- Stage bodies the user writes -----------------------------------------


@lv.runner
async def run(
    prompt: lv.PromptArtifact,
    case: lv.Case,
    cx,
) -> str:
    response = await cx.lm.complete(
        prompt=prompt.template.format(**case.input),
        max_tokens=64,
    )
    return response.text.strip()


@lv.scorer
async def score(output: str, case: lv.Case, cx) -> lv.Score:
    return lv.Score.exact_match(output, (case.target or {})["answer"])


# ---- Composition ----------------------------------------------------------


async def amain() -> None:
    pipeline = lv.optimize(
        seed=lv.PromptArtifact(template="Answer: {question}\nA:"),
        train=lv.cases.from_jsonl(str(FIXTURE), name="train", limit=6),
        val=lv.cases.from_jsonl(str(FIXTURE), name="val", limit=2),
        optimizer=lv.optimizers.gepa(population_size=8),
        environment=lv.environment.local(budget=lv.budget(usd=20)),
        runner=run,
        scorer=score,
    )

    # `.run()` raises NotImplementedError in the scaffold — once the engine
    # is wired, this returns an `Optimized[PromptArtifact]` typed result.
    print("pipeline composed; .run() awaiting engine wiring.")
    print("  optimizer :", pipeline.optimizer)  # type: ignore[attr-defined]
    print("  train     :", pipeline.train)       # type: ignore[attr-defined]


if __name__ == "__main__":
    try:
        asyncio.run(amain())
    except NotImplementedError as e:
        print(f"(expected) {e}")
