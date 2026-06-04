"""Durable public-seam optimize mechanics driver."""

import asyncio
import os

from msgspec import UNSET

from .._seam import (
    CodexCliRuntimeConfig,
    CommandRunnerStageConfig,
    MockLmRuntimeConfig,
    OpenAiLmRuntimeConfig,
    SeamClient,
    SeamExecutionContext,
    SeamServiceConfig,
    StageRunProposeRequest,
    StageRunRequest,
    effect_capability,
    proposer_stage_capability,
    resolve_codex_binary,
)
from .._seam._wire.refs import WireJsonField, WireJsonLeafArray, WireJsonLeafObject
from .._seam_worker import worker_argv_for_stage
from ..agent.codex import CodexAgent
from ..artifacts.prompt import PromptArtifact
from ..decorators import RegisteredStage
from ..json_value import JsonObject, JsonValue
from ..lm.config import LmConfig
from ..lm.mock import MockLm
from ..lm.openai import OpenAiLm
from ..optimizers.gepa import Gepa
from ..rubric import Rubric
from ..runtime import Runtime
from .receipts import (
    effect_cost_totals_from_stage_result,
    effect_receipts_from_stage_result,
    proposal_receipts_from_stage_result,
    sum_effect_cost_totals,
)
from .rewards import evaluate_reward_vector
from .scoring import mean_score
from .status import first_agent, unsupported_facts_for_runtime
from .types import (
    PlannedOptimizeCase,
    ProposerStageReport,
    SeamOptimizeReport,
    SeamStageAssessment,
)

PROMPT_SURFACE_FINGERPRINT = "fp_surface_sha256_python_prompt_template"
PROMPT_CHANGE_SCHEMA = "fp_schema_sha256_python_prompt_patch"


async def run_prompt_mechanics(
    *,
    seed: PromptArtifact,
    cases: list[PlannedOptimizeCase],
    runner: RegisteredStage[object, object],
    optimizer: Gepa,
    rubric: Rubric,
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
            lm=_lm_config(runtime, fallback_text=runner_text),
            stage=CommandRunnerStageConfig(
                argv=worker_argv_for_stage(runner, lm_model=_lm_model(runtime))
            ),
        )
    )
    assessments = []
    for index, case in enumerate(cases):
        result = await asyncio.to_thread(
            client.stage_run,
            StageRunRequest(
                request_id=f"stage-optimize-{index}",
                run_id=f"run_{run_id}",
                stage_call_id=f"sc_{run_id}_{index}",
                candidate="cand_seed",
                case=case.case_id,
                case_input=_case_input(seed, case),
            ),
        )
        if result.output.value is UNSET:
            raise TypeError("runner stage result must include text output value")
        output = result.output.value
        if not isinstance(output, str):
            raise TypeError("runner stage result output value must be text")
        score, rewards = await evaluate_reward_vector(
            rubric=rubric,
            output=output,
            case=case,
        )
        assessments.append(
            SeamStageAssessment(
                case_id=case.case_id,
                case_input=dict(case.input),
                case_target=dict(case.target) if case.target is not None else None,
                case_metadata=dict(case.metadata),
                case_split=case.split,
                output=output,
                score=score,
                rewards=rewards,
                effect_receipts=effect_receipts_from_stage_result(result),
                effect_costs=effect_cost_totals_from_stage_result(result),
            )
        )
    proposer_report = await _run_configured_proposer(
        optimizer=optimizer,
        runtime=runtime,
        run_id=run_id,
        assessments=assessments,
    )
    score = mean_score([assessment.score.value for assessment in assessments])
    effect_totals = sum_effect_cost_totals([assessment.effect_costs for assessment in assessments])
    return SeamOptimizeReport(
        seed_score=score,
        best_score=score,
        assessments=assessments,
        total_cost_usd=effect_totals.cost_usd,
        total_lm_tokens=effect_totals.lm_tokens,
        proposal_receipts=proposer_report.proposal_receipts,
        effect_receipts=proposer_report.effect_receipts,
        unsupported=unsupported_facts_for_runtime(runtime),
    )


async def _run_configured_proposer(
    *,
    optimizer: Gepa,
    runtime: Runtime,
    run_id: str,
    assessments: list[SeamStageAssessment],
) -> ProposerStageReport:
    propose = optimizer.propose
    if propose is None:
        return ProposerStageReport()
    if propose.kind != "function" or propose.stage is None:
        raise NotImplementedError(
            "this slice supports a function proposer (`Propose.fn(proposer)`); "
            f"got proposer kind {propose.kind!r}"
        )
    if propose.stage.role != "proposer":
        raise TypeError(
            f"the propose stage must be a @lv.proposer; got role {propose.stage.role!r}"
        )

    stage_call_id = f"sc_{run_id}_proposer_0"
    capability_fingerprint = "fp_cap_sha256_python_proposer"
    policy_fingerprint = "fp_policy_sha256_python_proposer"
    parent_candidate = "cand_seed"
    parent_workspace = _materialized_workspace_id(parent_candidate)
    agent_config = _agent_config(runtime)
    client = SeamClient(
        config=SeamServiceConfig(
            context=SeamExecutionContext(
                capability_fingerprint=capability_fingerprint,
                policy_fingerprint=policy_fingerprint,
                base_revision=f"rev_{run_id}",
            ),
            capability=proposer_stage_capability(
                capability_fingerprint=capability_fingerprint,
                policy_fingerprint=policy_fingerprint,
                surface_fingerprint=PROMPT_SURFACE_FINGERPRINT,
                change_schema=PROMPT_CHANGE_SCHEMA,
                candidate=parent_candidate,
                workspace=parent_workspace,
                jti=f"jti_{run_id}_python_proposer",
                stage_call_id=stage_call_id,
                allow_agent=agent_config is not None,
            ),
            agent=agent_config,
            lm=_lm_config(runtime, fallback_text=_runner_text(runtime)),
            stage=CommandRunnerStageConfig(
                argv=worker_argv_for_stage(propose.stage, lm_model=_lm_model(runtime))
            ),
        )
    )
    result = await asyncio.to_thread(
        client.stage_propose,
        StageRunProposeRequest(
            request_id=f"stage-propose-{run_id}-0",
            run_id=f"run_{run_id}",
            stage_call_id=stage_call_id,
            base_revision=f"rev_{run_id}",
            parent=parent_candidate,
            surface_fingerprint=PROMPT_SURFACE_FINGERPRINT,
            change_schema=PROMPT_CHANGE_SCHEMA,
            capability_fingerprint=capability_fingerprint,
            query_policy_fingerprint=policy_fingerprint,
            reflection_summary=_reflection_summary(assessments),
        ),
    )
    proposal_receipts = proposal_receipts_from_stage_result(result)
    if not proposal_receipts:
        raise RuntimeError("proposer stage result missing proposal_receipts")
    return ProposerStageReport(
        proposal_receipts=proposal_receipts,
        effect_receipts=effect_receipts_from_stage_result(result),
    )


def _reflection_summary(assessments: list[SeamStageAssessment]) -> str:
    if not assessments:
        return "seed candidate has not been assessed"
    fragments = []
    for assessment in assessments:
        fragments.append(f"{assessment.case_id}: score={assessment.score.value:.3f}")
        fragments.extend(
            f"{reward.id}: {reward.feedback}"
            for reward in assessment.rewards
            if reward.feedback
        )
    return "; ".join(fragments) or "seed assessment completed"


def _materialized_workspace_id(candidate_id: str) -> str:
    stem = candidate_id.removeprefix("cand_")
    sanitized = "".join(ch if ch.isalnum() or ch == "_" else "_" for ch in stem)
    return f"ws_{sanitized}_materialized"


def _case_input(seed: PromptArtifact, case: PlannedOptimizeCase) -> WireJsonField:
    value: JsonObject = dict(case.input)
    try:
        value["prompt"] = seed.template.format(**value)
    except KeyError:
        value["prompt"] = seed.template
    return _wire_leaf_object(value)


def _wire_leaf_object(value: JsonObject) -> WireJsonLeafObject:
    return {key: _wire_leaf_field(item) for key, item in value.items()}


def _wire_leaf_field(value: JsonValue) -> str | int | float | bool | None | WireJsonLeafArray:
    if value is None or isinstance(value, str | int | float | bool):
        return value
    if isinstance(value, list):
        if not all(item is None or isinstance(item, str | int | float | bool) for item in value):
            raise ValueError("runner case_input arrays must contain only JSON scalar values")
        return value
    raise ValueError("runner case_input fields must be JSON scalars or scalar arrays")


def _runner_text(runtime: Runtime) -> str:
    lm = _first_lm(runtime.lm)
    if isinstance(lm, MockLm) and lm.responses:
        return lm.responses[0]
    return "[mock]"


def _lm_config(
    runtime: Runtime, *, fallback_text: str
) -> MockLmRuntimeConfig | OpenAiLmRuntimeConfig:
    lm = _first_lm(runtime.lm)
    if isinstance(lm, MockLm):
        return MockLmRuntimeConfig(text=fallback_text)
    if isinstance(lm, OpenAiLm):
        return OpenAiLmRuntimeConfig(
            api_key_env=lm.api_key_env,
            base_url=lm.base_url,
            timeout_s=int(lm.timeout_s) if lm.timeout_s is not None else None,
            max_retries=lm.max_retries,
        )
    raise NotImplementedError(
        f"this slice supports mock and OpenAI LM runtime; got {type(lm).__name__}"
    )


def _lm_model(runtime: Runtime) -> str:
    return _first_lm(runtime.lm).model


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
