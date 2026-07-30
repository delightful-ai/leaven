"""Harbor-backed Leaven rollout helpers.

`agent_kit(agent=...)` builds a function-backed `lv.Rollout` that evaluates an
`AgentKitArtifact` by running one Harbor Trial of the chosen agent. The kit is
injected through the agent's real configuration surface (see
`leaven.x.harbor.agents`), selected by `placement`; the task working directory is
an explicit `workdir` parameter, never a hardcoded `/app`.
"""

import hashlib
import json
import os
import secrets
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
                trial_name=_trial_name(case.id, kit.candidate_id, kit=kit),
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


def _trial_name(
    case_id: str,
    candidate_id: str | None,
    *,
    kit: lv.AgentKitArtifact | None = None,
) -> str:
    """Build a Harbor trial dir name that never reuses leftover evidence.

    Harbor ``TrialPaths.mkdir`` uses ``exist_ok=True`` and never clears an
    existing trial directory. Reusing a name lets a later failed evaluation
    read a prior ``verifier/ctrf.json`` and poison GEPA rankings via
    ``ctrf_fraction``.

    Seam ``optimize.run`` currently collapses worker candidate refs to a
    per-case label, so ``candidate_id`` alone does not separate seed vs child
    kits. Hash kit content when available, and always append a per-invocation
    nonce so same-kit re-evaluations still get a fresh directory.
    """
    candidate = candidate_id or "seed"
    identity = f"{case_id}\0{candidate}"
    if kit is not None:
        identity = f"{identity}\0{kit.system_prompt}"
        for skill in kit.skills:
            identity = f"{identity}\0{skill.path}\0{skill.content}"
    digest = hashlib.sha256(identity.encode("utf-8")).hexdigest()[:10]
    nonce = secrets.token_hex(4)
    prefix = "".join(
        ch if ch.isalnum() or ch in "-_" else "_" for ch in f"{case_id}__{candidate}"
    )[:72]
    return f"{prefix}_{digest}_{nonce}"


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
    trajectory_path = trial_dir / "agent" / "trajectory.json"
    # Only trust on-disk CTRF when this trial produced an in-memory verifier
    # result. Harbor never clears reused trial dirs, so a failed attempt must
    # not inherit a prior evaluation's ``verifier/ctrf.json``.
    ctrf = (
        _read_ctrf(trial_dir / "verifier" / "ctrf.json") if verifier is not None else None
    )
    return HarborTrialOutcome(
        trial_dir=str(trial_dir),
        rewards=reward_map,
        ctrf=ctrf,
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
    "DEFAULT_TASK_KEY",
    "HarborTrialPlan",
    "TrialRunner",
    "agent_kit",
]
