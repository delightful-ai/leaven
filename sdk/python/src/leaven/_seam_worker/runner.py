"""Run one registered Python runner stage from a stage.run payload."""

from dataclasses import dataclass

from .._seam._wire.payloads import OutputRecord, RunnerRequest, StageRunRequest, StageRunResult
from .._seam._wire.refs import CandidateRef, CandidateRefRecord, CaseRef, CaseRefRecord
from ..artifacts.prompt import PromptArtifact
from ..case import InputCaseView
from ..decorators import RegisteredStage
from ..json_value import JsonObject, JsonValue
from .context import JsonRpcCallbackClient, rollout_context


async def run_runner_stage(
    stage: RegisteredStage[PromptArtifact, str],
    params: StageRunRequest,
    *,
    lm_model: str,
) -> StageRunResult:
    """Execute one target-free runner request and return a stage_run_result."""
    payload = params.payload
    if not isinstance(payload, RunnerRequest):
        raise TypeError(f"stage.run payload is not a runner role: {payload!r}")
    if payload.target_forbidden is not True:
        raise ValueError("runner stage payload must be target-free")
    if stage.role != "runner":
        raise ValueError(f"configured stage must be a runner; got {stage.role!r}")

    case_input = _prompt_runner_case_input(payload)
    candidate = _candidate_id(payload.candidate)
    case_id = _case_id(payload.case)
    stage_call_id = payload.stage_call_id
    prompt = PromptArtifact(template=case_input.prompt, candidate_id=candidate)
    case = InputCaseView(id=case_id, input=case_input.case_fields)
    callback = JsonRpcCallbackClient(lm_model=lm_model)
    cx = rollout_context(
        candidate_id=candidate,
        stage_call_id=stage_call_id,
        capability_fingerprint=payload.capability_fingerprint,
        lm_model=lm_model,
        callback=callback,
    )
    raw_output = await stage.func(prompt, case, cx)
    if not isinstance(raw_output, str):
        raise TypeError("runner stages must return str for the current text output contract")
    return StageRunResult(
        schema_version="leaven.stage_run.v1",
        message="stage_run_result",
        stage="runner",
        stage_call_id=stage_call_id,
        output=OutputRecord(
            kind="text",
            summary=f"runner output for {case_id}",
            value=raw_output.strip(),
            visibility="optimizer_visible",
            data_classes=["candidate.output"],
        ),
        effect_receipts=callback.effect_receipts(),
    )


def _candidate_id(value: CandidateRef) -> str:
    if isinstance(value, str):
        return value
    if isinstance(value, CandidateRefRecord):
        return value.id
    raise TypeError(f"unsupported candidate ref: {value!r}")


def _case_id(value: CaseRef) -> str:
    if isinstance(value, str):
        return value
    if isinstance(value, CaseRefRecord):
        return value.id
    raise TypeError(f"unsupported case ref: {value!r}")


@dataclass(frozen=True, slots=True)
class _PromptRunnerCaseInput:
    """Typed projection for the current PromptArtifact runner payload."""

    prompt: str
    case_fields: JsonObject


def _prompt_runner_case_input(payload: RunnerRequest) -> _PromptRunnerCaseInput:
    case_input = payload.case_input
    try:
        prompt = case_input["prompt"]
    except KeyError as error:
        raise ValueError("runner case_input must carry prompt") from error
    if not isinstance(prompt, str):
        raise TypeError("runner case_input.prompt must be a string")
    case_fields: dict[str, JsonValue] = {
        key: value for key, value in case_input.items() if key != "prompt"
    }
    return _PromptRunnerCaseInput(prompt=prompt, case_fields=case_fields)


__all__ = ["run_runner_stage"]
