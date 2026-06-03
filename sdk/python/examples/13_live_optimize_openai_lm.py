"""Example 13 -- live OpenAI LM from inside `lv.optimize(...).run()`.

This is a live-spend proof for Python SDK -> public seam -> Python runner
worker -> nested `leaven/lm.complete` -> configured OpenAI provider. It is
intentionally skipped by default.

The behavior-bearing proof is the `live_openai_lm` example project. Run only
when live OpenAI spend is intended:

    LEAVEN_LIVE_OPENAI=1 uv run --project examples/live_openai_lm live-openai-lm

Set `LEAVEN_OPENAI_MODEL` to override the default model.
"""

import os
import subprocess
from pathlib import Path


def main() -> None:
    """Delegate to the behavior-bearing example project command."""
    project = Path(__file__).parent / "live_openai_lm"
    env = os.environ.copy()
    env.pop("VIRTUAL_ENV", None)
    subprocess.run(
        ["uv", "run", "--project", str(project), "live-openai-lm"],
        check=True,
        env=env,
    )


if __name__ == "__main__":
    main()
