"""Example 15 -- live Codex agent-kit optimization on Terminal-Bench-2 via Harbor.

This is the live-spend proof that the Python SDK optimizes a Codex AGENT KIT (a
system prompt materialized as AGENTS.md plus skill files) on ONE real
Terminal-Bench-2 task: Python SDK -> `leaven/optimize.run` -> host GEPA loop with
AGENTIC Codex reflection -> Python runner worker -> ONE Harbor Trial per rollout,
with `@openai/codex` installed in-container solving the pinned `regex-log` task.

The target cutoff: a CHANGED kit authored by live Codex reflection from real
trial traces, APPLIED through the run graph and RE-EVALUATED onto the frontier,
beating the seed (verifier reward 0->1, or strictly better CTRF partial credit).
The live path is verified functional end to end (kit upload to the in-container
`AGENTS.md`, in-container codex solve, verifier reward/CTRF, the Git-backed kit
loop, durable run), but a live kit child that strictly beats the seed was NOT
demonstrated: a strong in-container model solves the chosen self-contained
Terminal-Bench-2 tasks regardless of the kit, so a weak-but-honest seed left no
headroom. That is a recorded headroom blocker, not an implementation gap; the
load-bearing cutoff proof is the deterministic no-spend mechanics test cited
below.

It is intentionally skipped by default. The behavior-bearing proof is the
`codex_terminal_bench` example project (its own uv project pinning
`harbor==0.13.1`). Run only when live Codex/Docker spend is intended:

    set -a; source ../../.env; set +a
    LEAVEN_CODEX_LIVE=1 LEAVEN_OPTIMIZE_TIMEOUT_S=7200 \
        LEAVEN_RUNS_ROOT=.leaven/release-runs \
        uv run python examples/15_live_optimize_codex_terminal_bench.py

Requirements (each prints an actionable skip message when missing): the
`LEAVEN_CODEX_LIVE=1` opt-in, a running Docker daemon (the trials run in
containers with internet access for the in-container codex install), and
`OPENAI_API_KEY` in the environment.

The deterministic, no-spend mechanics of this code path are proven by
`examples/codex_terminal_bench/tests/test_kit_optimization_mechanics.py`, which
drives the real served optimize path with a scripted fake codex reflection
binary and an explicit no-spend Harbor trial seam (no Docker, no live spend).
That deterministic test is the load-bearing cutoff proof; a live kit child that
strictly beats the seed was not yet demonstrated (a recorded headroom blocker,
see `docs/working-memory/gepa-over-seam-continuation.md`).
"""

import os
import shutil
import subprocess
from pathlib import Path

LIVE_ENV = "LEAVEN_CODEX_LIVE"


def _docker_available() -> bool:
    if shutil.which("docker") is None:
        return False
    try:
        result = subprocess.run(
            ["docker", "info"], capture_output=True, timeout=20, check=False
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


def main() -> None:
    """Skip with an actionable message, or delegate to the example project."""
    reason = _skip_reason()
    if reason is not None:
        print(f"skipped: {reason}")
        return
    project = Path(__file__).parent / "codex_terminal_bench"
    env = os.environ.copy()
    env.pop("VIRTUAL_ENV", None)
    subprocess.run(
        ["uv", "run", "--project", str(project), "codex-terminal-bench"],
        check=True,
        env=env,
    )


if __name__ == "__main__":
    main()
