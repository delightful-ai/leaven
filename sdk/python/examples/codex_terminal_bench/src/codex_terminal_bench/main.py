"""Command entrypoint for the live Codex agent-kit Terminal-Bench-2 proof.

This runs ONE real optimization: the Python SDK drives `lv.optimize(...).run()`
over the durable public seam, the host runs the real GEPA loop with agentic Codex
reflection, and each rollout runs a live Harbor Trial of the pinned
Terminal-Bench-2 task with Codex installed in-container. The cutoff is a kit
authored by live Codex reflection from real trial traces, applied through the
run graph and re-evaluated onto the frontier beating the seed.

It self-skips unless `LEAVEN_CODEX_LIVE=1` and Docker are available.
"""

import asyncio
import os
import shutil
import subprocess

from .output import print_optimization_outcome
from .scenario import LIVE_ENV, build_optimization, pinned_task_cases


def _docker_available() -> bool:
    if shutil.which("docker") is None:
        return False
    try:
        result = subprocess.run(
            ["docker", "info"],
            capture_output=True,
            timeout=20,
            check=False,
        )
    except (OSError, subprocess.TimeoutExpired):
        return False
    return result.returncode == 0


def _skip_reason() -> str | None:
    if os.environ.get(LIVE_ENV) != "1":
        return f"set {LIVE_ENV}=1 to run the live Codex Terminal-Bench-2 optimization"
    if not _docker_available():
        return "Docker is not available; start the Docker daemon to run the live trials"
    if not os.environ.get("OPENAI_API_KEY"):
        return "OPENAI_API_KEY is not set; `set -a; source ../../.env; set +a` before running"
    return None


async def amain() -> None:
    """Run or skip the live Codex agent-kit optimization based on operator opt-in."""
    reason = _skip_reason()
    if reason is not None:
        print(f"skipped: {reason}")
        return

    task_path = os.environ.get("LEAVEN_CODEX_TB_TASK", "regex-log")
    metric_calls = int(os.environ.get("LEAVEN_CODEX_TB_METRIC_CALLS", "8"))
    result = await build_optimization(
        cases=pinned_task_cases(task_path=task_path),
        metric_calls=metric_calls,
        minibatch_size=1,
        population_size=2,
    ).run()

    print_optimization_outcome(result)


def run() -> None:
    """Run the live proof from the project console entrypoint."""
    asyncio.run(amain())


__all__ = ["amain", "run"]
