"""Harbor-backed Leaven rollout helpers.

`agent_kit(agent=...)` builds a function-backed `lv.Rollout` that evaluates an
`AgentKitArtifact` by running one Harbor Trial of the chosen agent. The kit is
injected through the agent's real configuration surface (see
`leaven.x.harbor.agents`), selected by `placement`; the task working directory is
an explicit `workdir` parameter, never a hardcoded `/app`.
"""

import json
import os
import tempfile
from collections.abc import Awaitable, Callable
from dataclasses import dataclass
from pathlib import Path

import leaven as lv
from leaven.x.harbor._kit import materialize_agent_kit
from leaven.x.harbor._types import (
    CtrfEvidence,
    HarborAdapterError,
    HarborTrialOutcome,
    TokenEvidence,
)
from leaven.x.harbor.agents import resolve

DEFAULT_TASK_KEY = "harbor_task"


@dataclass(frozen=True)
class HarborTrialPlan:
    """Inputs needed to execute one Harbor trial for a materialized kit."""

    agent: str
    staging_dir: Path
    trials_dir: Path
    trial_name: str
    task_path: str
    model: str
    placement: str
    workdir: str = "/app"
    git_url: str | None = None
    git_commit_id: str | None = None
    api_key: str = ""
    api_key_env: str = "OPENAI_API_KEY"
    agent_env: dict[str, str] | None = None
    timeout_multiplier: float = 1.0


TrialRunner = Callable[[HarborTrialPlan], Awaitable[HarborTrialOutcome]]


def agent_kit(
    *,
    agent: str,
    model: str | None = None,
    placement: str | None = None,
    task_key: str = DEFAULT_TASK_KEY,
    trials_dir: str | Path = ".leaven/harbor-trials",
    workdir: str = "/app",
    git_url: str | None = None,
    git_commit_id: str | None = None,
    timeout_multiplier: float = 1.0,
    api_key_env: str | None = None,
    agent_env: dict[str, str] | None = None,
    trial_runner: TrialRunner | None = None,
) -> lv.Rollout:
    """Build a no-target Harbor rollout that evaluates an AgentKit with `agent`."""
    adapter = resolve(agent)
    resolved_model = model or adapter.default_model
    resolved_placement = placement or adapter.default_placement
    resolved_key_env = api_key_env or adapter.api_key_env
    if resolved_placement == "user" and adapter.user_prompt_mode == "unsupported_append_flag":
        raise HarborAdapterError(
            f"{adapter.key} user placement is disabled: Harbor renders "
            "--append-system-prompt without shell quoting, so multiword kit prompts "
            "can replace the task instruction. Use placement='repo'."
        )
    runner = trial_runner or _run_live_harbor_trial
    trials_root = Path(trials_dir)

    @lv.runner(id="leaven.x.harbor.rollout.agent_kit")
    async def run(
        kit: lv.AgentKitArtifact,
        case: lv.InputCaseView,
        cx: lv.RolloutContext,
    ) -> str:
        _ = cx
        task_path = _task_path_from_case(case, task_key=task_key)
        trials_root.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(prefix="leaven-harbor-kit-", dir=trials_root) as kit_tmp:
            staging = materialize_agent_kit(kit, Path(kit_tmp) / "kit")
            plan = HarborTrialPlan(
                agent=adapter.key,
                staging_dir=staging,
                trials_dir=trials_root,
                trial_name=_trial_name(case.id, kit.candidate_id),
                task_path=task_path,
                model=resolved_model,
                placement=resolved_placement,
                workdir=workdir,
                git_url=git_url,
                git_commit_id=git_commit_id,
                api_key=os.environ.get(resolved_key_env, ""),
                api_key_env=resolved_key_env,
                agent_env=agent_env,
                timeout_multiplier=timeout_multiplier,
            )
            outcome = await runner(plan)
        return outcome.encode()

    return lv.Rollout.fn(run)


def _task_path_from_case(case: lv.InputCaseView, *, task_key: str) -> str:
    value = case.input.get(task_key)
    if isinstance(value, dict) and isinstance(value.get("path"), str):
        return value["path"]
    if isinstance(value, str):
        return value
    raise TypeError(f"case input must carry `{task_key}` as a task path or local Harbor ref")


def _trial_name(case_id: str, candidate_id: str | None) -> str:
    candidate = candidate_id or "seed"
    safe = "".join(ch if ch.isalnum() or ch in "-_" else "_" for ch in f"{case_id}__{candidate}")
    return safe[:96]


async def _run_live_harbor_trial(plan: HarborTrialPlan) -> HarborTrialOutcome:
    """Run a live Harbor Trial lazily so importing Leaven does not require Harbor."""
    try:
        from harbor.models.trial.config import (  # noqa: PLC0415
            EnvironmentConfig,
            TaskConfig,
            TrialConfig,
        )
        from harbor.trial.trial import Trial  # noqa: PLC0415
    except ImportError as exc:
        raise HarborAdapterError(
            "Harbor is required to execute live Harbor rollouts; install with "
            "`pip install 'leaven[harbor]'` or pass `trial_runner=` for deterministic tests"
        ) from exc

    adapter = resolve(plan.agent)
    agent_config = adapter.agent_config(
        model=plan.model,
        placement=plan.placement,
        workdir=plan.workdir,
        staging_dir=plan.staging_dir,
        api_key=plan.api_key,
        agent_env=plan.agent_env,
    )
    config = TrialConfig(
        task=_task_config(plan, TaskConfig),
        trial_name=plan.trial_name,
        trials_dir=plan.trials_dir,
        timeout_multiplier=plan.timeout_multiplier,
        agent=agent_config,
        environment=EnvironmentConfig(),
    )
    trial = await Trial.create(config)
    result = await trial.run()
    return _outcome_from_result(result, trial_dir=plan.trials_dir / plan.trial_name)


def _task_config(plan: HarborTrialPlan, task_config_cls: type) -> object:
    if plan.git_url:
        return task_config_cls(
            git_url=plan.git_url,
            git_commit_id=plan.git_commit_id,
            path=Path(plan.task_path),
        )
    return task_config_cls(path=Path(plan.task_path))


def _outcome_from_result(result: object, *, trial_dir: Path) -> HarborTrialOutcome:
    verifier = getattr(result, "verifier_result", None)
    rewards = getattr(verifier, "rewards", None) if verifier is not None else None
    reward_map = {str(key): float(value) for key, value in (rewards or {}).items()}
    input_tokens = output_tokens = None
    cost_usd = None
    if hasattr(result, "compute_token_cost_totals"):
        input_tokens, _cache, output_tokens, cost_usd = result.compute_token_cost_totals()
    return HarborTrialOutcome(
        trial_dir=str(trial_dir),
        rewards=reward_map,
        ctrf=_trial_ctrf(trial_dir),
        verifier_output=_verifier_output(result, reward_map),
        trajectory_path=_trial_trajectory_path(trial_dir),
        tokens=TokenEvidence(input=input_tokens, output=output_tokens),
        cost_usd=cost_usd,
        exception=_exception_message(result),
    )


def _trial_ctrf(trial_dir: Path) -> CtrfEvidence | None:
    """Load CTRF from the trial root, or aggregate Harbor multi-step archives.

    Harbor `MultiStepTrial` relocates each step's verifier outputs into
    `steps/<name>/verifier/` and then removes the empty root `verifier/` mount
    dir. Reading only `verifier/ctrf.json` therefore drops all partial-credit
    evidence for multi-step tasks.
    """
    root = _read_ctrf(trial_dir / "verifier" / "ctrf.json")
    if root is not None:
        return root
    return _aggregate_step_ctrf(trial_dir)


def _aggregate_step_ctrf(trial_dir: Path) -> CtrfEvidence | None:
    passed = 0
    failed = 0
    total = 0
    failed_names: list[str] = []
    found = False
    for step_name in _step_names(trial_dir):
        evidence = _read_ctrf(trial_dir / "steps" / step_name / "verifier" / "ctrf.json")
        if evidence is None:
            continue
        found = True
        passed += evidence.passed
        failed += evidence.failed
        total += evidence.total
        for name in evidence.failed_names:
            if name not in failed_names:
                failed_names.append(name)
    if not found:
        return None
    return CtrfEvidence(
        passed=passed,
        failed=failed,
        total=total,
        failed_names=failed_names,
    )


def _trial_trajectory_path(trial_dir: Path) -> str | None:
    """Prefer root ATIF, else the latest Harbor multi-step agent archive."""
    root = trial_dir / "agent" / "trajectory.json"
    if root.is_file():
        return str(root)
    last: Path | None = None
    for step_name in _step_names(trial_dir):
        candidate = trial_dir / "steps" / step_name / "agent" / "trajectory.json"
        if candidate.is_file():
            last = candidate
    return str(last) if last is not None else None


def _step_names(trial_dir: Path) -> list[str]:
    result_path = trial_dir / "result.json"
    if result_path.is_file():
        try:
            data = json.loads(result_path.read_text(encoding="utf-8"))
        except (OSError, UnicodeError, json.JSONDecodeError):
            data = None
        if isinstance(data, dict):
            names = [
                str(step["step_name"])
                for step in data.get("step_results") or []
                if isinstance(step, dict) and isinstance(step.get("step_name"), str)
            ]
            if names:
                return names
    steps_dir = trial_dir / "steps"
    if not steps_dir.is_dir():
        return []
    return sorted(path.name for path in steps_dir.iterdir() if path.is_dir())


def _read_ctrf(path: Path) -> CtrfEvidence | None:
    if not path.is_file():
        return None

    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError):
        return None
    results = data.get("results", {}) if isinstance(data, dict) else {}
    summary = results.get("summary", {}) if isinstance(results, dict) else {}
    tests = results.get("tests", []) if isinstance(results, dict) else []
    if not isinstance(summary, dict) or not isinstance(tests, list):
        return None
    try:
        passed = int(summary.get("passed") or 0)
        failed = int(summary.get("failed") or 0)
        total = int(summary.get("tests") or passed + failed)
    except (TypeError, ValueError):
        return None
    failed_names = [
        str(test.get("name") or "unnamed")
        for test in tests
        if isinstance(test, dict) and test.get("status") not in {"passed", "skipped"}
    ]
    return CtrfEvidence(passed=passed, failed=failed, total=total, failed_names=failed_names)


def _verifier_output(result: object, rewards: dict[str, float]) -> str:
    lines = [f"verifier rewards: {rewards}"]
    exception = _exception_message(result)
    if exception:
        lines.append(f"trial exception: {exception}")
    return "\n".join(lines)


def _exception_message(result: object) -> str | None:
    exception = getattr(result, "exception_info", None)
    if exception is None:
        return None
    return str(getattr(exception, "exception_message", exception))


__all__ = [
    "DEFAULT_TASK_KEY",
    "HarborTrialPlan",
    "TrialRunner",
    "agent_kit",
]
