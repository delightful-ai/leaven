"""Run one registered Python runner stage from a stage.run payload."""

from .._seam._wire import JsonObject
from .._seam._wire.json_value import json_object
from .._seam._wire.payloads import RunnerRequest, StageRunRequest
from ..artifacts.prompt import PromptArtifact
from ..case import InputCaseView
from ..decorators import RegisteredStage
from .context import JsonRpcCallbackClient, rollout_context


async def run_runner_stage(
    stage: RegisteredStage[object, object],
    params: StageRunRequest,
    *,
    lm_model: str,
) -> JsonObject:
    """Execute one target-free runner request and return a stage_run_result."""
    payload = params.payload
    if not isinstance(payload, RunnerRequest):
        raise TypeError(f"stage.run payload is not a runner role: {payload!r}")
    if payload.target_forbidden is not True:
        raise ValueError("runner stage payload must be target-free")
    if stage.role != "runner":
        raise ValueError(f"configured stage must be a runner; got {stage.role!r}")

    case_input = json_object(payload.case_input)
    rendered_prompt = str(case_input["prompt"])
    candidate = _string_ref(payload.candidate, "candidate")
    case_id = _string_ref(payload.case, "case")
    stage_call_id = payload.stage_call_id
    prompt = PromptArtifact(template=rendered_prompt, candidate_id=candidate)
    view_input = {key: value for key, value in case_input.items() if key != "prompt"}
    case = InputCaseView(id=case_id, input=view_input)
    callback = JsonRpcCallbackClient(lm_model=lm_model)
    cx = rollout_context(
        candidate_id=candidate,
        stage_call_id=stage_call_id,
        lm_model=lm_model,
        callback=callback,
    )
    raw_output = await stage.func(prompt, case, cx)
    output = raw_output if isinstance(raw_output, str) else str(raw_output)
    return json_object(
        {
            "schema_version": "leaven.stage_run.v1",
            "message": "stage_run_result",
            "stage": "runner",
            "stage_call_id": stage_call_id,
            "output": {
                "kind": "text",
                "summary": f"runner output for {case_id}",
                "value": output.strip(),
                "visibility": "optimizer_visible",
                "data_classes": ["candidate.output"],
            },
            "effect_receipts": callback.effect_receipts_json(),
        }
    )


def _string_ref(value: object, field: str) -> str:
    if isinstance(value, str):
        return value
    candidate_id = getattr(value, "id", None)
    if isinstance(candidate_id, str):
        return candidate_id
    raise ValueError(f"stage.run runner payload field {field} must be a string")


__all__ = ["run_runner_stage"]
