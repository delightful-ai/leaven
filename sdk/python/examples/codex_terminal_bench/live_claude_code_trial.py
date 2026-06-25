"""Live proof: Claude Code solves one Harbor task through the generic adapter.

This runs ONE real Harbor Trial of the pinned Terminal-Bench-2 ``regex-log`` task
with Claude Code as the agent, driven entirely through the generic
``lv.x.harbor.rollout.agent_kit`` seam (no Codex-specific glue). The kit is
materialized into the task repo as ``CLAUDE.md`` plus project skills; this avoids
Harbor's unquoted ``--append-system-prompt`` path, which can split multiword
system prompts before Claude Code receives the task instruction.

It is the live counterpart to the deterministic
``tests/test_agent_kit_claude_code_uses_repo_placement_by_default``.

REQUIREMENTS (all live, real spend):
  - Docker running (Harbor builds the task container).
  - Claude Code auth, either:
      * ANTHROPIC_API_KEY exported, or
      * CLAUDE_CODE_OAUTH_TOKEN exported, or
      * the STS lab token at ~/.config/sts2-lab/claude-oauth-token.
  - Opt-in: LEAVEN_LIVE_CLAUDE_CODE=1.

RUN:
  LEAVEN_LIVE_CLAUDE_CODE=1 \
    uv run --project sdk/python/examples/codex_terminal_bench \
    python sdk/python/examples/codex_terminal_bench/live_claude_code_trial.py

It prints the structured HarborTrialOutcome (reward map, CTRF, tokens, cost,
trajectory path). Proving Claude Code *runs the task and produces real evidence*
is the goal; whether reward==1 depends on the model.
"""

import asyncio
import os
from datetime import UTC, datetime
from pathlib import Path

import leaven as lv

# Reuse the pinned Terminal-Bench-2 task identity from the Codex example so this
# changes only the agent, not the task.
from codex_terminal_bench.trial import TASK_GIT_COMMIT, TASK_GIT_URL

LIVE_ENV = "LEAVEN_LIVE_CLAUDE_CODE"
MODEL = "anthropic/claude-sonnet-4-6"
TRIALS_DIR = Path(".leaven") / "claude-code-tb-trials"
STS_OAUTH_TOKEN_PATH = Path.home() / ".config" / "sts2-lab" / "claude-oauth-token"

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


def _require_live() -> dict[str, str]:
    if os.environ.get(LIVE_ENV) != "1":
        raise SystemExit(f"set {LIVE_ENV}=1 to run this live Claude Code Harbor trial")
    if os.environ.get("ANTHROPIC_API_KEY"):
        return {}
    if token := os.environ.get("CLAUDE_CODE_OAUTH_TOKEN"):
        return {"CLAUDE_FORCE_OAUTH": "1", "CLAUDE_CODE_OAUTH_TOKEN": token}
    if STS_OAUTH_TOKEN_PATH.is_file():
        token = STS_OAUTH_TOKEN_PATH.read_text(encoding="utf-8").strip()
        if token:
            return {"CLAUDE_FORCE_OAUTH": "1", "CLAUDE_CODE_OAUTH_TOKEN": token}
    raise SystemExit(
        "Claude Code auth is not available. Export ANTHROPIC_API_KEY, export "
        "CLAUDE_CODE_OAUTH_TOKEN, or create ~/.config/sts2-lab/claude-oauth-token."
    )


async def main() -> None:
    agent_env = _require_live()

    rollout = lv.x.harbor.rollout.agent_kit(
        agent="claude-code",
        model=MODEL,
        placement="repo",  # Claude Code reads CLAUDE.md + .claude/skills in the task repo.
        trials_dir=TRIALS_DIR,
        git_url=TASK_GIT_URL,
        git_commit_id=TASK_GIT_COMMIT,
        agent_env=agent_env,
    )
    run_id = datetime.now(UTC).strftime("%Y%m%dT%H%M%SZ")
    case = lv.InputCaseView(
        id=f"claude_code_regex_log_{run_id}",
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
