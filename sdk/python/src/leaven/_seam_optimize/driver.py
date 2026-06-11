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
import os
from dataclasses import dataclass
from pathlib import Path
from types import FunctionType

from msgspec import UNSET, UnsetType

from .._errors import UnsupportedConfigurationError
from .._seam import (
    ArtifactRecord,
    CommandRunnerStageConfig,
    MockLmResponse,
    MockLmRuntimeConfig,
    OpenAiLmRuntimeConfig,
    OptimizeCase,
    OptimizerConfigDocument,
    OptimizeRunRequestDocument,
    OptimizeRunResultDocument,
    ReflectionLmConfig,
    SeamClient,
    SeamExecutionContext,
    SeamServiceConfig,
)
from .._seam.optimize_run import OptimizeSplit
from .._seam_worker import worker_argv_for_stage
from ..artifacts.prompt import PromptArtifact
from ..decorators import RegisteredStage
from ..lm.config import LmConfig
from ..lm.mock import MockLm
from ..lm.openai import OpenAiLm
from ..optimizers.gepa import Gepa
from ..rubric import RegisteredReward, Rubric
from ..runtime import Runtime
from .types import PlannedOptimizeCase

PROMPT_ARTIFACT_TYPE = "prompt"
PROMPT_ARTIFACT_SCHEMA = "fp_schema_sha256_prompt"
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
    seed: PromptArtifact,
    cases: list[PlannedOptimizeCase],
    runner: RegisteredStage[PromptArtifact, str],
    optimizer: Gepa,
    rubric: Rubric,
    run_id: str,
    runtime: Runtime,
    runs_root: str,
) -> OptimizeRunOutcome:
    """Drive one real optimization through `leaven/optimize.run`."""
    if not rubric.rewards:
        raise ValueError("the environment rubric must contain at least one reward")
    reward_names = tuple(_reward_name(reward) for reward in rubric.rewards)
    lm_model = _lm_model(runtime)
    client = SeamClient(
        config=SeamServiceConfig(
            context=SeamExecutionContext(
                capability_fingerprint=OPTIMIZE_CAPABILITY_FINGERPRINT,
                policy_fingerprint="fp_policy_sha256_python_optimize",
                base_revision=f"rev_{run_id}",
            ),
            lm=_lm_config(runtime),
            stage=CommandRunnerStageConfig(
                argv=worker_argv_for_stage(runner, lm_model=lm_model, reward_names=reward_names)
            ),
            optimize_runs_root=runs_root,
        )
    )
    document = _request_document(
        seed=seed,
        cases=cases,
        optimizer=optimizer,
        runtime=runtime,
        run_id=run_id,
    )
    result = await asyncio.to_thread(
        client.optimize_run,
        f"optimize-{run_id}",
        document,
    )
    return OptimizeRunOutcome(
        result=result,
        runs_root=runs_root,
        run_id=run_id,
        wire_run_id=document.run_id,
    )


def _request_document(
    *,
    seed: PromptArtifact,
    cases: list[PlannedOptimizeCase],
    optimizer: Gepa,
    runtime: Runtime,
    run_id: str,
) -> OptimizeRunRequestDocument:
    return OptimizeRunRequestDocument(
        schema_version="leaven.optimize_run.v1",
        message="optimize_run_request",
        run_id=f"run_{run_id}",
        seed=ArtifactRecord(
            artifact_type=PROMPT_ARTIFACT_TYPE,
            artifact_schema=PROMPT_ARTIFACT_SCHEMA,
            artifact={"template": seed.template},
        ),
        cases=[_wire_case(case) for case in cases],
        optimizer=_optimizer_config(optimizer, runtime),
        reflection=ReflectionLmConfig(model=_reflection_model(optimizer, runtime)),
        capability_fingerprint=OPTIMIZE_CAPABILITY_FINGERPRINT,
    )


def _wire_case(case: PlannedOptimizeCase) -> OptimizeCase:
    if case.target is None:
        raise ValueError(
            f"case {case.case_id!r} has no target; the optimize host scores against case targets"
        )
    return OptimizeCase(
        case=case.case_id,
        input=dict(case.input),
        target=dict(case.target),
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


def _reward_name(reward: RegisteredReward) -> str:
    """Return the reward's function name (stable across the worker module reload)."""
    func = reward.func
    if not isinstance(func, FunctionType):
        raise TypeError("rubric rewards require function-backed reward objects")
    return func.__name__


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


def default_runs_root() -> str:
    """Return the local runs root the SDK configures for durable optimize runs."""
    override = os.environ.get("LEAVEN_RUNS_ROOT")
    if override:
        return override
    return str(Path(".leaven") / "runs")


__all__ = ["OptimizeRunOutcome", "default_runs_root", "run_optimization"]
