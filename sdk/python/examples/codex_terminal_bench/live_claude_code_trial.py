"""Live proof: Claude Code solves one Harbor task through the generic adapter.

This runs ONE real Harbor Trial of the pinned Terminal-Bench-2 ``regex-log`` task
with Claude Code as the agent, driven entirely through the generic
``lv.x.harbor.rollout.agent_kit`` seam (no Codex-specific glue). The kit's system
prompt is injected via Claude Code's ``--append-system-prompt`` and skills via
``AgentConfig.skills`` (user-scope, workdir-independent) — exactly the path the
deterministic tests build, now executed for real.

It is the live counterpart to the deterministic
``tests/test_agent_kit_claude_code_uses_user_placement_by_default``.

REQUIREMENTS (all live, real spend):
  - Docker running (Harbor builds the task container).
  - ANTHROPIC_API_KEY exported (the in-container Claude Code authenticates with it).
  - Opt-in: LEAVEN_LIVE_CLAUDE_CODE=1.

RUN:
  set -a; source /Users/darin/src/personal/leaven/.env; set +a   # if your key lives there
  export ANTHROPIC_API_KEY=sk-ant-...
  LEAVEN_LIVE_CLAUDE_CODE=1 \
    uv run --project sdk/python/examples/codex_terminal_bench \
    python sdk/python/examples/codex_terminal_bench/live_claude_code_trial.py

It prints the structured HarborTrialOutcome (reward map, CTRF, tokens, cost,
trajectory path). Proving Claude Code *runs the task and produces real evidence*
is the goal; whether reward==1 depends on the model.
"""

import asyncio
import os
from pathlib import Path

import leaven as lv

# Reuse the pinned Terminal-Bench-2 task identity from the Codex example so this
# changes only the agent, not the task.
from codex_terminal_bench.trial import TASK_GIT_COMMIT, TASK_GIT_URL

LIVE_ENV = "LEAVEN_LIVE_CLAUDE_CODE"
MODEL = "anthropic/claude-sonnet-4-6"
TRIALS_DIR = Path(".leaven") / "claude-code-tb-trials"

# A deliberately helpful kit so Claude Code has a real chance on the task. The
# point of the proof is that the kit is injected and a real trial runs; the kit
# teaches a careful general method, never the task's specific answer.
SEED_KIT = lv.AgentKitArtifact(
    system_prompt=(
        "You are a careful command-line agent. Before writing a solution: read "
        "the task inputs fully, enumerate edge cases the task calls out, and test "
        "your answer against them before finishing. Prefer correctness over speed."
    ),
    skills=[],
)


def _require_live() -> str:
    if os.environ.get(LIVE_ENV) != "1":
        raise SystemExit(f"set {LIVE_ENV}=1 to run this live Claude Code Harbor trial")
    if not os.environ.get("ANTHROPIC_API_KEY"):
        raise SystemExit(
            "ANTHROPIC_API_KEY is not set; the in-container Claude Code needs it. "
            "Export it (or `set -a; source .env; set +a`) and re-run."
        )
    return os.environ["ANTHROPIC_API_KEY"]


async def main() -> None:
    _require_live()

    rollout = lv.x.harbor.rollout.agent_kit(
        agent="claude-code",
        model=MODEL,
        placement="user",  # workdir-independent: append-system-prompt + AgentConfig.skills
        trials_dir=TRIALS_DIR,
        git_url=TASK_GIT_URL,
        git_commit_id=TASK_GIT_COMMIT,
    )
    case = lv.InputCaseView(
        id="claude_code_regex_log",
        input={"harbor_task": {"path": "regex-log", "kind": "git"}},
    )

    print(f"running one live Claude Code Harbor trial ({MODEL}) on regex-log ...")
    encoded = await rollout.stage.func(SEED_KIT, case, None)  # type: ignore[union-attr,arg-type]
    outcome = lv.x.harbor.HarborTrialOutcome.decode(encoded)

    print("\n=== Claude Code Harbor trial outcome ===")
    print(f"  rewards:        {outcome.rewards}")
    print(f"  ctrf:           {outcome.ctrf}")
    print(f"  tokens:         {outcome.tokens}")
    print(f"  cost_usd:       {outcome.cost_usd}")
    print(f"  trajectory:     {outcome.trajectory_path}")
    print(f"  trial_dir:      {outcome.trial_dir}")
    if outcome.exception:
        print(f"  exception:      {outcome.exception}")
    print("\nClaude Code ran the task through the generic Harbor adapter.")


if __name__ == "__main__":
    asyncio.run(main())
