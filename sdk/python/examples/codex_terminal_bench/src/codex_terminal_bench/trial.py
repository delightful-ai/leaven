"""Run ONE Harbor Trial of the pinned Terminal-Bench-2 task for one agent kit.

The rollout materializes the candidate agent kit, then runs a single Harbor
`Trial` in a Docker container: Harbor installs `@openai/codex` in-container, the
`LeavenCodex` agent uploads the kit into the task working directory, Codex
solves the task, and the task's verifier writes a reward (1/0) plus a CTRF
per-test report. This module extracts the rollout evidence the optimizer scores
on: the verifier reward, the CTRF passed/total fraction, agent token/cost
totals, and the on-disk trajectory path.

Harbor spend rides inside this rollout function: it is reported back as rollout
evidence, not capability-gated through the public seam (an accepted V1 caveat
documented at the example boundary).
"""

import os
from dataclasses import dataclass
from pathlib import Path

from harbor.models.trial.config import AgentConfig, EnvironmentConfig, TaskConfig, TrialConfig
from harbor.trial.trial import Trial

from codex_terminal_bench.wire import CtrfReport, decode_ctrf

# Explicit no-spend test seam: when set, `run_trial` does not start Docker or
# Codex; it scores the materialized kit deterministically from its content. This
# is the deepest reachable no-spend cut for the rollout half of the loop (the
# served public-seam optimize path with a real agentic-reflection kit child),
# since the rollout runs in a worker subprocess where the live `Trial` cannot be
# monkeypatched. The seam is OFF by default, so the live example always runs a
# real Harbor Trial; only the deterministic test turns it on.
FAKE_TRIAL_ENV = "LEAVEN_CODEX_TB_FAKE_TRIAL"
# A kit whose AGENTS.md contains this marker "passes" the fake trial. The marker
# is what the deterministic fake-codex reflection writes into the evolved kit, so
# the fake trial rewards exactly the evolved child, never the seed.
FAKE_TRIAL_PASS_MARKER = "LEAVEN_TB_FAKE_PASS"
_FAKE_TRIAL_TESTS = 5

# The pinned Terminal-Bench-2 regex-log task: a fixed git URL + exact commit so
# the rollout evaluates one stable task definition across runs.
TASK_GIT_URL = "https://github.com/laude-institute/terminal-bench-2"
TASK_GIT_COMMIT = "2fd12b88aafdd04a52c298e3940bcb189f9766d6"
DEFAULT_TASK_PATH = "regex-log"

# The agent kit subclass selected through Harbor's AgentConfig import path.
LEAVEN_CODEX_IMPORT_PATH = "codex_terminal_bench.agent:LeavenCodex"
# Codex model run in-container. The kit is what the optimizer evolves; the model
# is held fixed so improvement is attributable to the kit, not the model.
DEFAULT_CODEX_MODEL = "openai/gpt-5.4-mini"

# The verifier reward key the task writes into its rewards map.
REWARD_KEY = "reward"


@dataclass(frozen=True)
class TrialOutcome:
    """The rollout evidence extracted from one Harbor Trial."""

    reward: float
    """The task verifier's pass/fail reward (1.0 or 0.0)."""
    ctrf_passed: int
    """Number of CTRF test cases that passed."""
    ctrf_total: int
    """Total number of CTRF test cases."""
    input_tokens: int | None
    output_tokens: int | None
    cost_usd: float | None
    trajectory_path: str | None
    """On-disk path to the agent's ATIF trajectory, when Harbor wrote one."""
    verifier_output: str
    """Human-readable verifier/CTRF summary used in scorer feedback."""

    @property
    def ctrf_fraction(self) -> float:
        """Fraction of CTRF tests that passed (0.0 when there are no tests)."""
        if self.ctrf_total <= 0:
            return 0.0
        return self.ctrf_passed / self.ctrf_total


@dataclass(frozen=True)
class TrialPlan:
    """The inputs to one rollout Trial: the kit dir and run-scoped paths."""

    agent_kit_dir: Path
    trials_dir: Path
    trial_name: str
    task_path: str = DEFAULT_TASK_PATH
    codex_model: str = DEFAULT_CODEX_MODEL
    openai_api_key: str = ""
    timeout_multiplier: float = 1.0


def build_trial_config(plan: TrialPlan) -> TrialConfig:
    """Build the Harbor TrialConfig for one pinned-task rollout of a kit."""
    return TrialConfig(
        task=TaskConfig(
            git_url=TASK_GIT_URL,
            git_commit_id=TASK_GIT_COMMIT,
            path=Path(plan.task_path),
        ),
        trial_name=plan.trial_name,
        trials_dir=plan.trials_dir,
        timeout_multiplier=plan.timeout_multiplier,
        agent=AgentConfig(
            import_path=LEAVEN_CODEX_IMPORT_PATH,
            model_name=plan.codex_model,
            kwargs={"agent_kit_dir": str(plan.agent_kit_dir)},
            env={"OPENAI_API_KEY": plan.openai_api_key},
        ),
        environment=EnvironmentConfig(),
    )


async def run_trial(plan: TrialPlan) -> TrialOutcome:
    """Run one Harbor Trial of the pinned task and extract rollout evidence.

    When the explicit no-spend test seam is enabled, score the materialized kit
    deterministically from its content instead of starting Docker/Codex.
    """
    if os.environ.get(FAKE_TRIAL_ENV) == "1":
        return _fake_trial_outcome(plan)
    config = build_trial_config(plan)
    trial = await Trial.create(config)
    result = await trial.run()
    return _outcome_from_result(result, trials_dir=plan.trials_dir, trial_name=plan.trial_name)


def _fake_trial_outcome(plan: TrialPlan) -> TrialOutcome:
    """Score a materialized kit deterministically (no Docker/Codex).

    The kit "passes" iff its AGENTS.md carries the pass marker the deterministic
    fake-codex reflection writes into evolved kits. The seed kit lacks it (reward
    0); an evolved child carries it (reward 1). So the assertion holds only if the
    kit child was actually applied and re-evaluated.
    """
    agents_md = plan.agent_kit_dir / "AGENTS.md"
    content = agents_md.read_text(encoding="utf-8") if agents_md.is_file() else ""
    passed = FAKE_TRIAL_PASS_MARKER in content
    reward = 1.0 if passed else 0.0
    ctrf_passed = _FAKE_TRIAL_TESTS if passed else 0
    verifier_output = (
        f"verifier reward: {reward:.0f}\n"
        f"CTRF {ctrf_passed}/{_FAKE_TRIAL_TESTS} tests passed"
    )
    if not passed:
        verifier_output += "; failing: test_regex_matches_dates"
    return TrialOutcome(
        reward=reward,
        ctrf_passed=ctrf_passed,
        ctrf_total=_FAKE_TRIAL_TESTS,
        input_tokens=0,
        output_tokens=0,
        cost_usd=0.0,
        trajectory_path=None,
        verifier_output=verifier_output,
    )


def _outcome_from_result(result, *, trials_dir: Path, trial_name: str) -> TrialOutcome:
    trial_dir = trials_dir / trial_name
    reward = _verifier_reward(result)
    ctrf_passed, ctrf_total, ctrf_summary = _ctrf_summary(trial_dir / "verifier" / "ctrf.json")
    input_tokens, _cache, output_tokens, cost_usd = result.compute_token_cost_totals()
    trajectory_path = trial_dir / "agent" / "trajectory.json"
    verifier_output = _verifier_output(result, reward=reward, ctrf_summary=ctrf_summary)
    return TrialOutcome(
        reward=reward,
        ctrf_passed=ctrf_passed,
        ctrf_total=ctrf_total,
        input_tokens=input_tokens,
        output_tokens=output_tokens,
        cost_usd=cost_usd,
        trajectory_path=str(trajectory_path) if trajectory_path.is_file() else None,
        verifier_output=verifier_output,
    )


def _verifier_reward(result) -> float:
    verifier = result.verifier_result
    if verifier is None or verifier.rewards is None:
        return 0.0
    rewards = verifier.rewards
    if REWARD_KEY not in rewards:
        return 0.0
    return float(rewards[REWARD_KEY])


def _ctrf_summary(ctrf_path: Path) -> tuple[int, int, str]:
    """Read the CTRF JSON report (passed/total) the task verifier wrote.

    The verifier writes a CTRF (Common Test Report Format) JSON whose
    `results.summary` carries `passed`/`failed`/`tests`. When the file is
    missing (the verifier never ran), the rollout has no per-test credit.
    """
    if not ctrf_path.is_file():
        return 0, 0, "no CTRF report (verifier did not run)"
    report = decode_ctrf(ctrf_path.read_bytes())
    summary = report.results.summary
    passed = summary.passed
    total = summary.tests if summary.tests else passed + summary.failed
    failed_names = _failed_test_names(report)
    detail = f"CTRF {passed}/{total} tests passed"
    if failed_names:
        detail += f"; failing: {', '.join(failed_names)}"
    return passed, total, detail


def _failed_test_names(report: CtrfReport) -> list[str]:
    """Return the names of failing CTRF tests (no internals, just names).

    Only test names are surfaced so scorer feedback names which checks failed
    without embedding the task's solution or hidden test internals (the TB2
    canary requirement).
    """
    return [
        test.name for test in report.results.tests if test.status not in {"passed", "skipped"}
    ]


def _verifier_output(result, *, reward: float, ctrf_summary: str) -> str:
    lines = [f"verifier reward: {reward:.0f}", ctrf_summary]
    if result.exception_info is not None:
        lines.append(f"trial exception: {result.exception_info.exception_message}")
    return "\n".join(lines)


__all__ = [
    "DEFAULT_CODEX_MODEL",
    "DEFAULT_TASK_PATH",
    "LEAVEN_CODEX_IMPORT_PATH",
    "REWARD_KEY",
    "TASK_GIT_COMMIT",
    "TASK_GIT_URL",
    "TrialOutcome",
    "TrialPlan",
    "build_trial_config",
    "run_trial",
]
