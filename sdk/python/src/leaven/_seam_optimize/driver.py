"""Lower an `lv.optimize(...)` composition into one `leaven/optimize.run` call.

This driver composes the seed, environment, optimizer, and runtime into a single
locked `leaven/optimize.run` request against the spawned `leaven seam serve
--stdio` host. The host runs the real GEPA loop, dispatching runner and scorer
stages back to the configured Python worker over `leaven/stage.run`. The driver
configures that worker (the registered runner plus the rubric reward ids), the
runtime LM provider, and a runs root the client knows, then returns the typed
result document the facade projects into `Optimized`.
"""

import asyncio
import functools
import os
import shutil
import tempfile
from dataclasses import dataclass
from pathlib import Path
from types import FunctionType

from msgspec import UNSET, UnsetType

from .._errors import UnsupportedConfigurationError
from .._seam import (
    CodexCliRuntimeConfig,
    CommandRunnerStageConfig,
    MockLmResponse,
    MockLmRuntimeConfig,
    OpenAiLmRuntimeConfig,
    OptimizeCase,
    OptimizerConfigDocument,
    OptimizeRunRequestDocument,
    OptimizeRunResultDocument,
    ReflectionAgenticConfig,
    ReflectionConfig,
    ReflectionLmConfig,
    SeamClient,
    SeamExecutionContext,
    SeamServiceConfig,
)
from .._seam.optimize_run import OptimizeSplit
from .._seam.resolve import resolve_codex_binary
from .._seam_worker import worker_argv_for_stage
from ..decorators import RegisteredStage
from ..lm.config import LmConfig
from ..lm.mock import MockLm
from ..lm.openai import OpenAiLm
from ..optimizers.gepa import Gepa
from ..rubric import RegisteredReward, Rubric
from ..runtime import Runtime
from .artifact_projection import OptimizeSeed, SeedProjection, project_seed
from .types import PlannedOptimizeCase

OPTIMIZE_CAPABILITY_FINGERPRINT = "fp_cap_sha256_python_optimize"


@dataclass(frozen=True)
class OptimizeRunOutcome:
    """The host optimize.run result plus the durable run dir used for readback."""

    result: OptimizeRunResultDocument
    runs_root: str
    run_id: str
    wire_run_id: str
    """The `run_<slug>` id the host persists the durable run dir under."""


async def run_optimization(
    *,
    seed: OptimizeSeed,
    cases: list[PlannedOptimizeCase],
    runner: RegisteredStage[OptimizeSeed, str],
    optimizer: Gepa,
    rubric: Rubric,
    run_id: str,
    runtime: Runtime,
    runs_root: str,
) -> OptimizeRunOutcome:
    """Drive one real optimization through `leaven/optimize.run`."""
    if not rubric.rewards:
        raise ValueError("the environment rubric must contain at least one reward")
    reward_ids = tuple(_reward_id(reward) for reward in rubric.rewards)
    projection = project_seed(seed)
    lm_model = _lm_model(runtime)
    agent_config = _agent_config(optimizer, reflection_kind=projection.reflection_kind)
    client = SeamClient(
        config=SeamServiceConfig(
            context=SeamExecutionContext(
                capability_fingerprint=OPTIMIZE_CAPABILITY_FINGERPRINT,
                policy_fingerprint="fp_policy_sha256_python_optimize",
                base_revision=f"rev_{run_id}",
            ),
            lm=_lm_config(runtime),
            agent=agent_config,
            stage=CommandRunnerStageConfig(
                argv=worker_argv_for_stage(runner, lm_model=lm_model, reward_ids=reward_ids)
            ),
            optimize_runs_root=runs_root,
        )
    )
    document = _request_document(
        projection=projection,
        cases=cases,
        optimizer=optimizer,
        runtime=runtime,
        run_id=run_id,
    )
    result = await asyncio.to_thread(
        functools.partial(
            client.optimize_run,
            f"optimize-{run_id}",
            document,
            timeout_s=_optimize_timeout_s(),
        )
    )
    return OptimizeRunOutcome(
        result=result,
        runs_root=runs_root,
        run_id=run_id,
        wire_run_id=document.run_id,
    )


def _request_document(
    *,
    projection: SeedProjection,
    cases: list[PlannedOptimizeCase],
    optimizer: Gepa,
    runtime: Runtime,
    run_id: str,
) -> OptimizeRunRequestDocument:
    return OptimizeRunRequestDocument(
        schema_version="leaven.optimize_run.v1",
        message="optimize_run_request",
        run_id=f"run_{run_id}",
        seed=projection.artifact,
        cases=[_wire_case(case) for case in cases],
        optimizer=_optimizer_config(optimizer, runtime),
        reflection=_reflection_config(projection.reflection_kind, optimizer, runtime),
        capability_fingerprint=OPTIMIZE_CAPABILITY_FINGERPRINT,
    )


def _reflection_config(
    reflection_kind: str,
    optimizer: Gepa,
    runtime: Runtime,
) -> ReflectionConfig:
    """Build the wire reflection config for the seed's reflection kind.

    The prompt path reflects with an LM model name; the agent-kit path reflects
    agentically through a configured agent runtime (the wire carries only the
    kind; the agent runtime is service-configured).
    """
    if reflection_kind == "agentic":
        if optimizer.reflection_lm is not None:
            raise UnsupportedConfigurationError(
                "gepa(reflection_lm=...) does not apply to an AgentKitArtifact seed; "
                "the agent-kit path reflects agentically. Pass "
                "gepa(reflection_agent=lv.agent.codex(...)) instead."
            )
        if optimizer.reflection_agent is None:
            raise UnsupportedConfigurationError(
                "optimizing an AgentKitArtifact seed requires an agentic reflection "
                "runtime; pass gepa(reflection_agent=lv.agent.codex(transport='cli', "
                "model=...)) so the host can evolve the kit."
            )
        return ReflectionAgenticConfig()
    if optimizer.reflection_agent is not None:
        raise UnsupportedConfigurationError(
            "gepa(reflection_agent=...) applies only to an AgentKitArtifact seed; "
            "a PromptArtifact seed reflects with an lm model. Remove reflection_agent "
            "or pass reflection_lm."
        )
    return ReflectionLmConfig(model=_reflection_model(optimizer, runtime))


def _agent_config(
    optimizer: Gepa,
    *,
    reflection_kind: str,
) -> CodexCliRuntimeConfig | None:
    """Lower the optimizer's reflection agent into a Codex CLI service config.

    Only the agentic kit path configures a host agent runtime; the prompt path
    leaves the host agent unset. The reflection agent must be a Codex CLI agent
    (`lv.agent.codex(transport='cli')`): the host's agentic reflector runs Codex
    in a materialized workspace to evolve the kit.
    """
    if reflection_kind != "agentic":
        return None
    agent = optimizer.reflection_agent
    if agent is None:
        raise UnsupportedConfigurationError(
            "agentic reflection requires gepa(reflection_agent=lv.agent.codex(...))"
        )
    if agent.transport != "cli":
        raise UnsupportedConfigurationError(
            "agent-kit reflection requires a Codex CLI agent; pass "
            f"lv.agent.codex(transport='cli', ...); got transport {agent.transport!r}"
        )
    codex_bin = _resolve_codex_bin(agent.bin_path_env)
    timeout_s = int(agent.timeout_s) if agent.timeout_s is not None else 600
    if agent.codex_home is not None:
        # Explicit operator override: use it verbatim, no HOME isolation.
        codex_home, home_dir = agent.codex_home, None
    else:
        codex_home, home_dir = _isolated_codex_home()
    return CodexCliRuntimeConfig(
        codex_bin=codex_bin,
        model=agent.model,
        timeout_s=timeout_s,
        codex_home=codex_home,
        home_dir=home_dir,
        bypass_approvals_and_sandbox=agent.approval_mode == "bypass",
    )


def _resolve_codex_bin(bin_path_env: str | None) -> str:
    """Resolve the Codex CLI binary path for the host reflection agent."""
    if bin_path_env is not None:
        path = os.environ.get(bin_path_env)
        if not path:
            raise UnsupportedConfigurationError(
                f"gepa(reflection_agent=lv.agent.codex(bin_path_env={bin_path_env!r})) "
                f"requires {bin_path_env} to point at the codex binary"
            )
        return path
    return resolve_codex_binary()


def _isolated_codex_home() -> tuple[str, str]:
    """Prepare a run-scoped `(codex_home, home_dir)` isolated from the operator.

    The host's reflection codex runs on the operator's machine, where it pulls
    context from two roots:

    - `$CODEX_HOME` -> `AGENTS.md` + `config.toml` (the operator's codex doctrine);
    - `$HOME` -> the skill registry `~/.agents/.skill-lock.json` and
      `~/.codex/superpowers`, i.e. the operator's whole personal skill arsenal.

    Isolating only `CODEX_HOME` severs the doctrine but leaves the `$HOME`-rooted
    skills, so codex still reaches for e.g. the "superpowers" workflow. We isolate
    both: the fresh home is the new `HOME`, `CODEX_HOME` is `<home>/.codex` with a
    copied `auth.json` (preserving the existing subscription/login). The reflection
    then sees only codex's built-in skills and the already-materialized workspace
    (kit) skills -- a reproducible surface independent of the operator's machine.

    The home lives under the runs root, not a self-deleting temp dir, because codex
    writes its session trajectory under `$CODEX_HOME/sessions`: a disposable home
    would erase the reflection trajectory we want to keep. Operator runs-root
    retention governs cleanup. An operator who needs their own codex config passes
    `lv.agent.codex(codex_home=...)` explicitly to opt out.
    """
    source = Path(os.environ.get("CODEX_HOME") or (Path.home() / ".codex"))
    homes_root = Path(default_runs_root()) / "codex-homes"
    homes_root.mkdir(parents=True, exist_ok=True)
    home = Path(tempfile.mkdtemp(prefix="codex-home-", dir=homes_root))
    codex_home = home / ".codex"
    codex_home.mkdir(parents=True, exist_ok=True)
    auth = source / "auth.json"
    if auth.is_file():
        destination = codex_home / "auth.json"
        shutil.copyfile(auth, destination)
        destination.chmod(0o600)
    return str(codex_home), str(home)


def _wire_case(case: PlannedOptimizeCase) -> OptimizeCase:
    # A case may legitimately carry no target: a rollout-judged task (e.g. a
    # benchmark verifier) scores from the rollout output, not a held answer. The
    # wire `target` field is required but may be JSON null, so a `None` target
    # rides as null and the scorer simply never reads a target.
    return OptimizeCase(
        case=case.case_id,
        input=dict(case.input),
        target=dict(case.target) if case.target is not None else None,
        metadata=dict(case.metadata) if case.metadata else UNSET,
        split=_wire_split(case.split),
    )


def _wire_split(split: str | None) -> OptimizeSplit | UnsetType:
    match split:
        case None:
            return UNSET
        case "train":
            return "train"
        case "validation" | "val":
            return "validation"
        case "test":
            return "test"
        case other:
            raise UnsupportedConfigurationError(
                f"unsupported case split {other!r}; the optimize host accepts "
                "train/validation/test"
            )


def _optimizer_config(optimizer: Gepa, runtime: Runtime) -> OptimizerConfigDocument:
    _refuse_unsupported_gepa(optimizer)
    _refuse_unsupported_budget(runtime)
    metric_calls = _metric_calls(runtime)
    usd_micro = _max_cost_usd_micro(runtime)
    return OptimizerConfigDocument(
        max_metric_calls=metric_calls,
        objective=optimizer.objective,
        population_size=optimizer.population_size,
        minibatch_size=optimizer.minibatch_size,
        max_cost_usd_micro=usd_micro if usd_micro is not None else UNSET,
    )


def _refuse_unsupported_gepa(optimizer: Gepa) -> None:
    """Refuse GEPA knobs that have no `leaven/optimize.run` route in V1.

    These fields would silently have no effect if accepted, so each is refused
    naming what the V1 optimize seam actually supports: population_size,
    minibatch_size, objective ("instance"), and lm reflection.
    """
    if optimizer.frontier is not None:
        raise UnsupportedConfigurationError(
            "gepa(frontier=...) has no leaven/optimize.run route in V1; "
            "the host uses the reference per-case Pareto frontier. V1 honors "
            "population_size, minibatch_size, objective='instance', and lm reflection."
        )
    if optimizer.parent_selector != "round_robin":
        raise UnsupportedConfigurationError(
            f"gepa(parent_selector={optimizer.parent_selector!r}) has no "
            "leaven/optimize.run route in V1; the host uses the reference parent "
            "selection. V1 honors population_size, minibatch_size, "
            "objective='instance', and lm reflection."
        )
    if optimizer.max_iterations is not None:
        raise UnsupportedConfigurationError(
            "gepa(max_iterations=...) has no leaven/optimize.run route in V1; "
            "the run is bounded by budget metric_calls (and an optional usd "
            "ceiling). V1 honors population_size, minibatch_size, "
            "objective='instance', and lm reflection."
        )
    if optimizer.reflect is not None:
        raise UnsupportedConfigurationError(
            "gepa(reflect=...) reflection-stage overrides have no "
            "leaven/optimize.run route in V1; the host reflects with the "
            "configured lm reflection model. V1 honors population_size, "
            "minibatch_size, objective='instance', and lm reflection."
        )
    if optimizer.propose is not None:
        raise UnsupportedConfigurationError(
            "gepa(propose=...) proposer-stage overrides have no "
            "leaven/optimize.run route in V1; the host runs the built-in GEPA "
            "proposer. V1 honors population_size, minibatch_size, "
            "objective='instance', and lm reflection."
        )
    if optimizer.reflection_lm is not None and not isinstance(optimizer.reflection_lm, MockLm | OpenAiLm):
        raise UnsupportedConfigurationError(
            "gepa(reflection_lm=...) on the optimize path supports a mock or "
            f"OpenAI LM config; got {type(optimizer.reflection_lm).__name__}. "
            "V1 reflection is an lm model name."
        )


def _reflection_model(optimizer: Gepa, runtime: Runtime) -> str:
    if optimizer.reflection_lm is not None:
        return optimizer.reflection_lm.model
    return _lm_model(runtime)


def _refuse_unsupported_budget(runtime: Runtime) -> None:
    """Refuse budget axes that have no `leaven/optimize.run` route in V1.

    The V1 optimize budget is `metric_calls` (with an optional `usd` cost
    ceiling). `calls`, `lm_tokens`, `wall_seconds`, and `concurrent_calls` would
    silently have no effect on the host loop, so each is refused loudly when set
    rather than dropped.
    """
    budget = runtime.budget
    if budget is None:
        return
    declared = {
        "calls": budget.calls,
        "lm_tokens": budget.lm_tokens,
        "wall_seconds": budget.wall_seconds,
        "concurrent_calls": budget.concurrent_calls,
    }
    unsupported = [name for name, value in declared.items() if value is not None]
    if unsupported:
        axes = ", ".join(unsupported)
        raise UnsupportedConfigurationError(
            f"lv.budget({axes}=...) has no leaven/optimize.run route in V1; the "
            "optimize budget is metric_calls (with an optional usd cost ceiling). "
            "Remove the unsupported budget axes."
        )


def _metric_calls(runtime: Runtime) -> int:
    budget = runtime.budget
    if budget is None or budget.metric_calls is None:
        raise UnsupportedConfigurationError(
            "a GEPA optimize run requires a metric-call budget; pass "
            "lv.budget(metric_calls=N) (N >= 1) to lv.runtime(...). The V1 "
            "optimize budget is metric_calls (with an optional usd ceiling)."
        )
    if budget.metric_calls < 1:
        raise UnsupportedConfigurationError("budget metric_calls must be at least 1")
    return budget.metric_calls


def _max_cost_usd_micro(runtime: Runtime) -> int | None:
    budget = runtime.budget
    if budget is None or budget.usd is None:
        return None
    micro = round(budget.usd * 1_000_000)
    if micro < 1:
        raise UnsupportedConfigurationError(
            "budget usd ceiling must be at least one micro-dollar (0.000001)"
        )
    return micro


def _lm_config(runtime: Runtime) -> MockLmRuntimeConfig | OpenAiLmRuntimeConfig:
    lm = _first_lm(runtime.lm)
    if isinstance(lm, MockLm):
        responses = tuple(MockLmResponse(text=text) for text in lm.responses)
        return MockLmRuntimeConfig(responses=responses or None)
    if isinstance(lm, OpenAiLm):
        return OpenAiLmRuntimeConfig(
            api_key_env=lm.api_key_env,
            base_url=lm.base_url,
            timeout_s=int(lm.timeout_s) if lm.timeout_s is not None else None,
            max_retries=lm.max_retries,
        )
    raise UnsupportedConfigurationError(
        f"this slice supports mock and OpenAI LM runtime; got {type(lm).__name__}"
    )


def _reward_id(reward: RegisteredReward) -> str:
    """Return the reward id used to reload the rubric in the worker process.

    Ids are the durable reward identity. Function `__name__` collides when
    imports or factory wrappers share a body name, which silently collapsed
    multi-reward vectors on worker reload.
    """
    func = reward.func
    if not isinstance(func, FunctionType):
        raise TypeError("rubric rewards require function-backed reward objects")
    return reward.id


def _lm_model(runtime: Runtime) -> str:
    return _first_lm(runtime.lm).model


def _first_lm(value: LmConfig | list[LmConfig] | dict[str, LmConfig]) -> LmConfig:
    if isinstance(value, list):
        if not value:
            raise ValueError("runtime.lm list must not be empty")
        return value[0]
    if isinstance(value, dict):
        if not value:
            raise ValueError("runtime.lm dict must not be empty")
        return next(iter(value.values()))
    return value


_DEFAULT_OPTIMIZE_TIMEOUT_S = 600


def _optimize_timeout_s() -> int:
    """Client wall-clock timeout for one `leaven/optimize.run` dispatch.

    A live GEPA run (sequential solves plus slow reasoning-model reflection) can
    exceed the default. `LEAVEN_OPTIMIZE_TIMEOUT_S` raises the operator ceiling
    for those runs without changing the default for deterministic runs.
    """
    override = os.environ.get("LEAVEN_OPTIMIZE_TIMEOUT_S")
    if not override:
        return _DEFAULT_OPTIMIZE_TIMEOUT_S
    seconds = int(override)
    if seconds < 1:
        raise UnsupportedConfigurationError("LEAVEN_OPTIMIZE_TIMEOUT_S must be a positive integer")
    return seconds


def default_runs_root() -> str:
    """Return the local runs root the SDK configures for durable optimize runs."""
    override = os.environ.get("LEAVEN_RUNS_ROOT")
    if override:
        return override
    return str(Path(".leaven") / "runs")


__all__ = ["OptimizeRunOutcome", "default_runs_root", "run_optimization"]
