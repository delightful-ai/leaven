"""Run one registered Python runner stage from a stage.run payload."""

from collections.abc import Mapping
from typing import Any

from ..artifacts.prompt import PromptArtifact
from ..case import InputCaseView
from ..decorators import RegisteredStage
from .context import JsonRpcCallbackClient, rollout_context


async def run_runner_stage(
    stage: RegisteredStage[Any, Any],
    params: Mapping[str, Any],
    *,
    lm_model: str,
) -> dict[str, Any]:
    """Execute one target-free runner request and return a stage_run_result."""
    payload = params["payload"]
    if payload.get("role") != "runner":
        raise ValueError(f"stage.run payload is not a runner role: {payload!r}")
    if payload.get("target_forbidden") is not True:
        raise ValueError("runner stage payload must be target-free")
    if stage.role != "runner":
        raise ValueError(f"configured stage must be a runner; got {stage.role!r}")

    case_input = dict(payload["case_input"])
    rendered_prompt = str(case_input["prompt"])
    prompt = PromptArtifact(template=rendered_prompt, candidate_id=payload["candidate"])
    view_input = {key: value for key, value in case_input.items() if key != "prompt"}
    case = InputCaseView(id=payload["case"], input=view_input)
    callback = JsonRpcCallbackClient(lm_model=lm_model)
    cx = rollout_context(
        candidate_id=payload["candidate"],
        stage_call_id=payload["stage_call_id"],
        lm_model=lm_model,
        callback=callback,
    )
    raw_output = await stage.func(prompt, case, cx)
    output = raw_output if isinstance(raw_output, str) else str(raw_output)
    return {
        "schema_version": "leaven.stage_run.v1",
        "message": "stage_run_result",
        "stage": "runner",
        "stage_call_id": payload["stage_call_id"],
        "output": {
            "kind": "text",
            "summary": f"runner output for {payload['case']}",
            "value": output.strip(),
            "visibility": "optimizer_visible",
            "data_classes": ["candidate.output"],
        },
        "effect_receipts": callback.effect_receipts_json(),
    }


__all__ = ["run_runner_stage"]
