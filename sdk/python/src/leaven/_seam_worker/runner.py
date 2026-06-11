"""Run one registered Python runner stage from a stage.run payload.

The optimize host dispatches a runner stage with the candidate material projected
under one key in `case_input`: a `candidate_template` string for a prompt
artifact, or a `candidate_agent_kit` flat wire artifact for an agent-kit
artifact. This module reconstructs the typed artifact the registered runner sees
from whichever key the host sent, so one runner worker serves both artifact
types (the registered `@lv.runner` declares which artifact type it accepts).
"""

from .._seam._wire.payloads import OutputRecord, RunnerRequest, StageRunRequest, StageRunResult
from .._seam._wire.refs import CandidateRef, CandidateRefRecord, CaseRef, CaseRefRecord
from ..artifacts.agent_kit import AgentKitArtifact
from ..artifacts.prompt import PromptArtifact
from ..case import InputCaseView
from ..decorators import RegisteredStage
from ..json_value import JsonObject
from .context import JsonRpcCallbackClient, rollout_context

# Candidate payload keys the host projects each candidate revision under.
_PROMPT_CANDIDATE_KEY = "candidate_template"
_AGENT_KIT_CANDIDATE_KEY = "candidate_agent_kit"


async def run_runner_stage(
    stage: RegisteredStage[object, str],
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

    candidate = _candidate_id(payload.candidate)
    case_id = _case_id(payload.case)
    stage_call_id = payload.stage_call_id
    artifact, case_fields = _runner_artifact(payload, candidate_id=candidate)
    case = InputCaseView(id=case_id, input=case_fields)
    callback = JsonRpcCallbackClient(lm_model=lm_model)
    cx = rollout_context(
        candidate_id=candidate,
        stage_call_id=stage_call_id,
        capability_fingerprint=payload.capability_fingerprint,
        lm_model=lm_model,
        callback=callback,
    )
    raw_output = await stage.func(artifact, case, cx)
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


def _runner_artifact(
    payload: RunnerRequest,
    *,
    candidate_id: str,
) -> tuple[PromptArtifact | AgentKitArtifact, JsonObject]:
    """Reconstruct the typed candidate artifact plus the target-free case input.

    The host's runner payload carries the candidate material under exactly one
    artifact-specific key (`candidate_template` for a prompt, `candidate_agent_kit`
    for an agent kit) and the target-free case input under `case_input`. The
    worker reconstructs the typed artifact the registered runner consumes from
    whichever key the host sent.
    """
    case_input = payload.case_input
    case_fields = _case_fields(case_input)
    if _PROMPT_CANDIDATE_KEY in case_input:
        template = case_input[_PROMPT_CANDIDATE_KEY]
        if not isinstance(template, str):
            raise TypeError(f"runner case_input.{_PROMPT_CANDIDATE_KEY} must be a string")
        return PromptArtifact(template=template, candidate_id=candidate_id), case_fields
    if _AGENT_KIT_CANDIDATE_KEY in case_input:
        wire = case_input[_AGENT_KIT_CANDIDATE_KEY]
        if not isinstance(wire, dict):
            raise TypeError(f"runner case_input.{_AGENT_KIT_CANDIDATE_KEY} must be a JSON object")
        kit = AgentKitArtifact.from_wire_artifact(wire, candidate_id=candidate_id)
        return kit, case_fields
    raise ValueError(
        f"runner case_input must carry a candidate under `{_PROMPT_CANDIDATE_KEY}` "
        f"or `{_AGENT_KIT_CANDIDATE_KEY}`"
    )


def _case_fields(case_input: JsonObject) -> JsonObject:
    try:
        nested = case_input["case_input"]
    except KeyError as error:
        raise ValueError("runner case_input must carry case_input") from error
    if not isinstance(nested, dict):
        raise TypeError("runner case_input.case_input must be a JSON object")
    return dict(nested)


__all__ = ["run_runner_stage"]
