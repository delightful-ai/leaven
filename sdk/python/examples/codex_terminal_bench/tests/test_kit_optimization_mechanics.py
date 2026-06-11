"""Deterministic no-spend proof of the Codex agent-kit optimization mechanics.

This drives the SAME composition as the live example (`build_optimization`,
`run`, `verifier`, `ctrf`) over the real `leaven seam serve --stdio` host and the
real GEPA loop with real agentic Git-program reflection, but with two no-spend
substitutions that are the deepest cut reachable through the served public seam:

  1. A deterministic fake-codex binary stands in for the host's agentic
     reflection runtime (the real `FakeAgentRuntime` is `#[cfg(test)]`-only in
     `leaven-seam-service` and is NOT reachable through the served-CLI service
     config, so the served path's only deterministic agentic option is a scripted
     codex binary). The fake-codex rewrites the materialized kit's
     `repos/agent_kit/system_prompt.md` to carry the pass marker, exactly as a
     real Codex reflection would author an improved kit.

  2. The example's explicit no-spend trial seam (`LEAVEN_CODEX_TB_FAKE_TRIAL`)
     scores the materialized kit from its content instead of starting Docker and
     Codex in a container. The rollout runs in a worker subprocess where the live
     Harbor `Trial` cannot be monkeypatched, so the seam is the only reachable
     no-spend cut for the rollout half of the loop.

Everything else is real: the served `leaven/optimize.run` dispatch, the host GEPA
loop, the Git-backed kit materialization and revision readback, the worker
runner/scorer dispatch, the rubric, and frontier admission. The seed kit lacks
the pass marker (reward 0); the fake-codex authors a child carrying it (reward 1).
So the assertion holds only if a CHANGED kit child was APPLIED through the run
graph and RE-EVALUATED onto the frontier, beating the seed.
"""

import os
import stat
import sys
from pathlib import Path

import leaven as lv
import pytest

from codex_terminal_bench.scenario import build_optimization, pinned_task_cases
from codex_terminal_bench.trial import FAKE_TRIAL_ENV, FAKE_TRIAL_PASS_MARKER

# The kit-relative path the host's kit loop materializes the system prompt at
# (the `repos/agent_kit` layout in leaven-seam-service plus `system_prompt.md`).
_KIT_SYSTEM_PROMPT_PATH = "repos/agent_kit/system_prompt.md"


def _write_fake_codex(tmp_path: Path) -> Path:
    """Write a deterministic fake-codex binary that evolves the kit.

    It parses `--output-last-message <path>` from argv (the Leaven Codex CLI
    runtime always passes it), rewrites the materialized kit system prompt to
    carry the pass marker, and writes a final message. Codex runs with the
    materialized reflection workspace as its working directory, so the kit path
    is resolved relative to the current directory.
    """
    script = tmp_path / "fake-codex"
    script.write_text(
        "#!/usr/bin/env python3\n"
        "import sys\n"
        "from pathlib import Path\n"
        f"MARKER = {FAKE_TRIAL_PASS_MARKER!r}\n"
        f"KIT = {_KIT_SYSTEM_PROMPT_PATH!r}\n"
        "argv = sys.argv[1:]\n"
        "last_message = None\n"
        "for i, arg in enumerate(argv):\n"
        "    if arg == '--output-last-message' and i + 1 < len(argv):\n"
        "        last_message = argv[i + 1]\n"
        "# Consume the rendered task from stdin (the reflection brief).\n"
        "sys.stdin.read()\n"
        "kit = Path(KIT)\n"
        "if kit.is_file():\n"
        "    prior = kit.read_text(encoding='utf-8')\n"
        "    evolved = (\n"
        "        prior.rstrip()\n"
        "        + '\\n\\nWhen working a task, first study the inputs, then write a\\n'\n"
        "        + 'precise solution and verify it before finishing. ' + MARKER + '\\n'\n"
        "    )\n"
        "    kit.write_text(evolved, encoding='utf-8')\n"
        "if last_message:\n"
        "    Path(last_message).write_text('Evolved the agent kit.', encoding='utf-8')\n"
        "sys.exit(0)\n",
        encoding="utf-8",
    )
    script.chmod(script.stat().st_mode | stat.S_IEXEC | stat.S_IRWXU)
    return script


@pytest.mark.asyncio
async def test_kit_child_is_applied_and_re_evaluated_onto_the_frontier(
    tmp_path: Path,
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Cutoff (no-spend): a changed kit child beats the seed on the frontier.

    Drives the real served optimize path; fails if the kit child is never
    applied or never re-evaluated (the seed would remain best at reward 0).
    """
    fake_codex = _write_fake_codex(tmp_path)
    trials_dir = tmp_path / "trials"
    runs_root = tmp_path / "runs"
    monkeypatch.setenv(FAKE_TRIAL_ENV, "1")
    monkeypatch.setenv("LEAVEN_CODEX_BIN", str(fake_codex))
    monkeypatch.setenv("LEAVEN_CODEX_TB_TRIALS_DIR", str(trials_dir))
    monkeypatch.setenv("LEAVEN_RUNS_ROOT", str(runs_root))
    monkeypatch.setenv("OPENAI_API_KEY", "sk-fake-not-used-in-fake-trial")

    result = await build_optimization(
        cases=pinned_task_cases(),
        metric_calls=8,
        minibatch_size=1,
        population_size=2,
    ).run()

    seed = next(c for c in result.frontier if c.parent_id is None)
    seed_score = seed.summary_score or 0.0
    best_score = result.best.summary_score or 0.0

    # The seed kit (no marker) scores 0; the applied, re-evaluated child scores 1.
    assert seed_score == pytest.approx(0.0)
    assert result.best.id != seed.id, "best must be a child, not the seed"
    assert best_score > seed_score, "the re-evaluated child must beat the seed"

    # The winning candidate is an agent kit whose evolved AGENTS.md carries the
    # marker the fake-codex authored — proof the kit content actually changed.
    best_kit = result.best.artifact
    assert isinstance(best_kit, lv.AgentKitArtifact)
    assert FAKE_TRIAL_PASS_MARKER in best_kit.system_prompt
    assert FAKE_TRIAL_PASS_MARKER not in _as_kit(seed.artifact).system_prompt


def _as_kit(artifact: object) -> lv.AgentKitArtifact:
    assert isinstance(artifact, lv.AgentKitArtifact)
    return artifact


def test_fake_codex_helper_is_executable(tmp_path: Path) -> None:
    """Sanity: the fake-codex stand-in is written executable for the runtime."""
    script = _write_fake_codex(tmp_path)
    assert os.access(script, os.X_OK)
    assert sys.executable  # interpreter resolvable for the shebang line
