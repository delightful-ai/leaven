"""Command entrypoint for the live OpenAI LM optimization proof."""

from __future__ import annotations

import asyncio
import json

from live_openai_lm.config import LIVE_ENV, LiveOpenAiConfig
from live_openai_lm.output import valid_live_lm_output
from live_openai_lm.scenario import optimize_with_live_openai


async def amain() -> None:
    """Run or skip the live OpenAI proof based on explicit operator opt-in."""
    config = LiveOpenAiConfig.from_env()
    if not config.enabled:
        print(f"skipped: set {LIVE_ENV}=1 to run the live OpenAI LM proof")
        return

    result = await optimize_with_live_openai(config)
    assessment = result.assessment("case_live_openai_lm_001")
    reward_output = assessment.rewards[0].output
    assert reward_output is not None
    assert isinstance(reward_output.value, str)
    value = json.loads(reward_output.value)

    assert result.best.summary_score == 1.0
    assert valid_live_lm_output(value)

    print("run id:          ", result.run_id)
    print("best score:      ", f"{result.best.summary_score:.3f}")
    print("lm receipt:      ", value["receipt"])
    print("lm tokens:       ", value["usage"]["total_tokens"])
    print("cost status:     ", result.summary.cost_status)


def run() -> None:
    """Run the live proof from the project console entrypoint."""
    asyncio.run(amain())


__all__ = ["amain", "run"]
