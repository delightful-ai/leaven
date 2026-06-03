"""Leaven scenario definition for the live OpenAI LM proof."""

import json

import leaven as lv

from live_openai_lm.config import EXPECTED_TEXT, LiveOpenAiConfig
from live_openai_lm.output import valid_live_lm_output


@lv.runner
async def run(prompt: lv.PromptArtifact, case: lv.Case, cx: lv.RolloutContext) -> str:
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
    return 1.0 if valid_live_lm_output(value) else 0.0


async def optimize_with_live_openai(config: LiveOpenAiConfig) -> lv.Optimized[lv.PromptArtifact]:
    """Run the live OpenAI-backed optimize mechanics path."""
    return await lv.optimize(
        seed=lv.PromptArtifact(
            template=f"Reply with exactly {EXPECTED_TEXT}. No punctuation, no extra words."
        ),
        environment=lv.Environment(
            task=lv.Task(
                name="live-openai-lm",
                cases=[
                    lv.Case(
                        id="live-openai-lm-001",
                        input={"question": "Can OpenAI run through Leaven?"},
                        target={"answer": EXPECTED_TEXT},
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
                model=config.model,
                api_key_env=config.api_key_env,
                timeout_s=120,
                max_retries=1,
            ),
            budget=lv.budget(usd=5),
        ),
    ).run()


__all__ = ["exact", "optimize_with_live_openai", "run"]
