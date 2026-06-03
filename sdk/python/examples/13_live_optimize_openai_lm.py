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

from live_openai_lm import amain

if __name__ == "__main__":
    asyncio.run(amain())
