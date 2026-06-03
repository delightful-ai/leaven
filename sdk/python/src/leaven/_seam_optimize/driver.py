"""Durable public-seam optimize mechanics driver."""

from __future__ import annotations

import asyncio
import os
from typing import Any

from .._seam import (
    CodexCliRuntimeConfig,
    CommandRunnerStageConfig,
    MockLmRuntimeConfig,
    SeamClient,
    SeamExecutionContext,
    SeamServiceConfig,
    StageRunRequest,
    effect_capability,
    resolve_codex_binary,
)
from .._seam_worker import worker_argv_for_stage
from ..agent.codex import CodexAgent
from ..artifacts.prompt import PromptArtifact
from ..decorators import RegisteredStage
from ..lm.config import LmConfig
from ..lm.mock import MockLm
from ..runtime import Runtime
from .scoring import exact_answer_score, mean_score
from .status import first_agent, unsupported_facts_for_runtime
from .types import SeamOptimizeReport, SeamStageAssessment


async def run_prompt_mechanics(
    *,
    seed: PromptArtifact,
    cases: list[dict[str, Any]],
    runner: RegisteredStage[Any, Any],
    run_id: str,
    runtime: Runtime,
) -> SeamOptimizeReport:
    """Run the current prompt slice through durable `leaven/stage.run`.

    This is mechanics evidence for the durable server route. The configured
    stage runner is a checked-in Python worker process that dispatches the
    registered `@lv.runner` and services `cx.lm.complete` over the active
    public-seam callback loop; it does not yet perform optimizer search.
    """
    runner_text = _runner_text(runtime)
    agent_config = _agent_config(runtime)
    capability_fingerprint = "fp_cap_sha256_python_optimize"
    policy_fingerprint = "fp_policy_sha256_python_optimize"
    client = SeamClient(
        config=SeamServiceConfig(
            context=SeamExecutionContext(
                capability_fingerprint=capability_fingerprint,
                policy_fingerprint=policy_fingerprint,
                base_revision=f"rev_{run_id}",
            ),
            capability=(
                effect_capability(
                    capability_fingerprint=capability_fingerprint,
                    policy_fingerprint=policy_fingerprint,
                    candidate="cand_seed",
                    workspace="ws_seed_materialized",
                    jti=f"jti_{run_id}_python_optimize",
                    stage_call_id=f"sc_{run_id}_agent",
                )
                if agent_config is not None
                else None
            ),
            agent=agent_config,
            lm=MockLmRuntimeConfig(text=runner_text),
            stage=CommandRunnerStageConfig(argv=worker_argv_for_stage(runner)),
        )
    )
    assessments = []
    for index, case in enumerate(cases):
        result = await asyncio.to_thread(
            client.request,
            StageRunRequest(
                request_id=f"stage-optimize-{index}",
                run_id=f"run_{run_id}",
                stage_call_id=f"sc_{run_id}_{index}",
                candidate="cand_seed",
                case=case["case_id"],
                case_input=_case_input(seed, case),
            ).to_json_rpc(),
        )
        output = result["output"]["value"]
        assessments.append(
            SeamStageAssessment(
                case_id=case["case_id"],
                output=output,
                score=exact_answer_score(output, case.get("target")),
            )
        )
    score = mean_score([assessment.score for assessment in assessments])
    return SeamOptimizeReport(
        seed_score=score,
        best_score=score,
        assessments=assessments,
        unsupported=unsupported_facts_for_runtime(runtime),
    )


def _case_input(seed: PromptArtifact, case: dict[str, Any]) -> dict[str, Any]:
    value = dict(case["input"])
    try:
        value["prompt"] = seed.template.format(**value)
    except KeyError:
        value["prompt"] = seed.template
    return value


def _runner_text(runtime: Runtime) -> str:
    lm = _first_lm(runtime.lm)
    if isinstance(lm, MockLm) and lm.responses:
        return lm.responses[0]
    return "[mock]"


def _agent_config(runtime: Runtime) -> CodexCliRuntimeConfig | None:
    agent = first_agent(runtime.agent)
    if agent is None:
        return None
    if not isinstance(agent, CodexAgent):
        raise NotImplementedError(
            f"this slice supports Codex agent runtime; got {type(agent).__name__}"
        )
    if agent.transport != "cli":
        raise NotImplementedError("this slice supports Codex CLI transport for agent callbacks")
    codex_bin = (
        _env_binary(agent.bin_path_env)
        if agent.bin_path_env is not None
        else resolve_codex_binary()
    )
    return CodexCliRuntimeConfig(
        codex_bin=codex_bin,
        model=agent.model,
        timeout_s=max(1, int(agent.timeout_s or 180)),
        bypass_approvals_and_sandbox=agent.approval_mode == "bypass",
    )


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


def _env_binary(env_name: str) -> str:
    value = os.environ.get(env_name)
    if value is None:
        raise ValueError(f"{env_name} is not set")
    return value


__all__ = ["run_prompt_mechanics"]
