"""Harbor-backed Leaven rollout helpers."""

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

DEFAULT_CODEX_MODEL = "openai/gpt-5.4-mini"
DEFAULT_TASK_KEY = "harbor_task"
LEAVEN_CODEX_IMPORT_PATH = "leaven.x.harbor:LeavenCodex"


@dataclass(frozen=True)
class HarborTrialPlan:
    """Inputs needed to execute one Harbor trial for a materialized kit."""

    agent_kit_dir: Path
    trials_dir: Path
    trial_name: str
    task_path: str
    codex_model: str = DEFAULT_CODEX_MODEL
    openai_api_key: str = ""
    timeout_multiplier: float = 1.0
    workdir: str = "/app"


TrialRunner = Callable[[HarborTrialPlan], Awaitable[HarborTrialOutcome]]


def codex_agent_kit(
    *,
    model: str = DEFAULT_CODEX_MODEL,
    task_key: str = DEFAULT_TASK_KEY,
    trials_dir: str | Path = ".leaven/harbor-trials",
    workdir: str = "/app",
    timeout_multiplier: float = 1.0,
    openai_api_key_env: str = "OPENAI_API_KEY",
    trial_runner: TrialRunner | None = None,
) -> lv.Rollout:
    """Build a no-target Harbor Codex rollout for Leaven AgentKit artifacts."""
    runner = trial_runner or _run_live_harbor_trial
    trials_root = Path(trials_dir)

    @lv.runner(id="leaven.x.harbor.rollout.codex_agent_kit")
    async def run(
        kit: lv.AgentKitArtifact,
        case: lv.InputCaseView,
        cx: lv.RolloutContext,
    ) -> str:
        _ = cx
        task_path = _task_path_from_case(case, task_key=task_key)
        trials_root.mkdir(parents=True, exist_ok=True)
        with tempfile.TemporaryDirectory(prefix="leaven-harbor-kit-", dir=trials_root) as kit_tmp:
            kit_dir = materialize_agent_kit(kit, Path(kit_tmp) / "kit")
            plan = HarborTrialPlan(
                agent_kit_dir=kit_dir,
                trials_dir=trials_root,
                trial_name=_trial_name(case.id, kit.candidate_id),
                task_path=task_path,
                codex_model=model,
                openai_api_key=os.environ.get(openai_api_key_env, ""),
                timeout_multiplier=timeout_multiplier,
                workdir=workdir,
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
            AgentConfig,
            EnvironmentConfig,
            TaskConfig,
            TrialConfig,
        )
        from harbor.trial.trial import Trial  # noqa: PLC0415
    except ImportError as exc:
        raise HarborAdapterError(
            "Harbor is required to execute live Harbor rollouts; install the Harbor "
            "adapter dependency or pass a trial_runner for deterministic tests"
        ) from exc

    config = TrialConfig(
        task=TaskConfig(path=Path(plan.task_path)),
        trial_name=plan.trial_name,
        trials_dir=plan.trials_dir,
        timeout_multiplier=plan.timeout_multiplier,
        agent=AgentConfig(
            import_path=LEAVEN_CODEX_IMPORT_PATH,
            model_name=plan.codex_model,
            kwargs={"agent_kit_dir": str(plan.agent_kit_dir), "workdir": plan.workdir},
            env={"OPENAI_API_KEY": plan.openai_api_key},
        ),
        environment=EnvironmentConfig(),
    )
    trial = await Trial.create(config)
    result = await trial.run()
    return _outcome_from_result(result, trial_dir=plan.trials_dir / plan.trial_name)


def _outcome_from_result(result: object, *, trial_dir: Path) -> HarborTrialOutcome:
    verifier = getattr(result, "verifier_result", None)
    rewards = getattr(verifier, "rewards", None) if verifier is not None else None
    reward_map = {str(key): float(value) for key, value in (rewards or {}).items()}
    input_tokens = output_tokens = None
    cost_usd = None
    if hasattr(result, "compute_token_cost_totals"):
        input_tokens, _cache, output_tokens, cost_usd = result.compute_token_cost_totals()
    trajectory_path = trial_dir / "agent" / "trajectory.json"
    return HarborTrialOutcome(
        trial_dir=str(trial_dir),
        rewards=reward_map,
        ctrf=_read_ctrf(trial_dir / "verifier" / "ctrf.json"),
        verifier_output=_verifier_output(result, reward_map),
        trajectory_path=str(trajectory_path) if trajectory_path.is_file() else None,
        tokens=TokenEvidence(input=input_tokens, output=output_tokens),
        cost_usd=cost_usd,
        exception=_exception_message(result),
    )


def _read_ctrf(path: Path) -> CtrfEvidence | None:
    if not path.is_file():
        return None

    data = json.loads(path.read_text(encoding="utf-8"))
    results = data.get("results", {}) if isinstance(data, dict) else {}
    summary = results.get("summary", {}) if isinstance(results, dict) else {}
    tests = results.get("tests", []) if isinstance(results, dict) else []
    passed = int(summary.get("passed") or 0)
    failed = int(summary.get("failed") or 0)
    total = int(summary.get("tests") or passed + failed)
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
    "DEFAULT_CODEX_MODEL",
    "DEFAULT_TASK_KEY",
    "LEAVEN_CODEX_IMPORT_PATH",
    "HarborTrialPlan",
    "TrialRunner",
    "codex_agent_kit",
]
