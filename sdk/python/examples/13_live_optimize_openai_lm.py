"""Example 13 -- live OpenAI LM from inside `lv.optimize(...).run()`.

This is a live-spend proof for Python SDK -> public seam -> Python runner
worker -> nested `leaven/lm.complete` -> configured OpenAI provider. It is
intentionally skipped by default.

Run only when live OpenAI spend is intended:

    LEAVEN_LIVE_OPENAI=1 uv run python examples/13_live_optimize_openai_lm.py

Set `LEAVEN_OPENAI_MODEL` to override the default model.
"""

from __future__ import annotations

import asyncio
import json
import os

import leaven as lv

EXPECTED = "LEAVEN_LIVE_LM_OK"


@lv.runner
async def run(prompt: lv.PromptArtifact, case: lv.InputCaseView, cx: lv.RolloutContext) -> str:
    """Call the configured live LM through the active stage seam."""
    _ = case
    reply = await cx.lm.complete(
        prompt=prompt.template,
        temperature=0.0,
        max_tokens=16,
        input_classes=["public"],
    )
    return json.dumps(
        {
            "text": reply.text.strip(),
            "receipt": reply.receipt.receipt_id,
            "usage": reply.usage,
            "cost_usd": reply.cost_usd,
            "model": reply.model,
        },
        sort_keys=True,
    )


@lv.reward
async def exact(output: str, case: lv.ScoringCaseView, cx: lv.RubricContext) -> float:
    """Score the live LM output and receipt projection."""
    _ = (case, cx)
    value = json.loads(output)
    return 1.0 if _valid_live_lm_output(value) else 0.0


def _valid_live_lm_output(value: dict[str, object]) -> bool:
    usage = value.get("usage")
    return (
        value.get("text") == EXPECTED
        and value.get("receipt") == "lmrec_completion"
        and isinstance(usage, dict)
        and int(usage.get("total_tokens", 0)) > 0
    )


async def amain() -> None:
    if os.environ.get("LEAVEN_LIVE_OPENAI") != "1":
        print("skipped: set LEAVEN_LIVE_OPENAI=1 to run the live OpenAI LM proof")
        return

    model = os.environ.get("LEAVEN_OPENAI_MODEL", "gpt-4.1-mini")
    result = await lv.optimize(
        seed=lv.PromptArtifact(
            template=f"Reply with exactly {EXPECTED}. No punctuation, no extra words."
        ),
        environment=lv.Environment(
            task=lv.Task(
                name="live-openai-lm",
                cases=[
                    lv.Case(
                        id="live-openai-lm-001",
                        input={"question": "Can OpenAI run through Leaven?"},
                        target={"answer": EXPECTED},
                        split="train",
                    )
                ],
            ),
            rollout=lv.Rollout.fn(run),
            rubric=lv.Rubric([exact]),
        ),
        optimizer=lv.optimizers.gepa(population_size=1),
        runtime=lv.runtime(
            workspace=lv.workspace.local(),
            lm=lv.lm.openai(
                model=model,
                api_key_env=os.environ.get("LEAVEN_OPENAI_API_KEY_ENV", "OPENAI_API_KEY"),
                timeout_s=120,
                max_retries=1,
            ),
            budget=lv.budget(usd=5),
        ),
    ).run()

    assessment = result.assessment("case_live_openai_lm_001")
    value = json.loads(str(assessment.output))
    assert result.best.summary_score == 1.0
    assert _valid_live_lm_output(value)
    print("run id:          ", result.run_id)
    print("best score:      ", f"{result.best.summary_score:.3f}")
    print("lm receipt:      ", value["receipt"])
    print("lm tokens:       ", value["usage"]["total_tokens"])
    print("cost status:     ", result.summary.cost_status)


if __name__ == "__main__":
    asyncio.run(amain())
