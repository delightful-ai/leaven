"""Run the registered Python rubric for one scorer stage dispatch.

The optimize host dispatches a scorer stage to the worker with a `ScoreContext`
payload (the runner output plus a `target_handle`). The worker reads the case
target/input/metadata through capability-gated `leaven/case.*` callbacks (the
host serves them with receipts during scorer dispatch), runs the rubric's
`@lv.reward` functions over the output, and returns the typed reward-vector
score the optimizer selects on.
"""

from msgspec import UNSET

from .._seam._wire.json_value import json_object
from .._seam._wire.payloads import (
    OutputRecord,
    ScorerRequest,
    StageRewardFact,
    StageRunRequest,
    StageRunResult,
    StageScore,
)
from .._seam._wire.refs import (
    CandidateRef,
    CandidateRefRecord,
    CaseRef,
    CaseRefRecord,
    WireJsonLiteralDepth8,
)
from .._seam.plans import CaseField, CaseLoadRequest
from .._stage_runtime import CallbackRubricContext
from ..case import ScoringCaseView
from ..contexts import RubricContext
from ..json_value import JsonObject
from ..rubric import RewardValue, Rubric
from .context import JsonRpcCallbackClient


async def run_scorer_stage(
    rubric: Rubric,
    params: StageRunRequest,
    *,
    lm_model: str,
) -> StageRunResult:
    """Execute one scorer dispatch and return a reward-vector stage_run_result."""
    payload = params.payload
    if not isinstance(payload, ScorerRequest):
        raise TypeError(f"stage.run payload is not a scorer role: {payload!r}")
    if not rubric.rewards:
        raise ValueError("scorer worker requires a rubric with at least one reward")

    output = _output_text(payload.output)
    candidate = _candidate_id(payload.candidate)
    case_id = _case_id(payload.case)
    callback = JsonRpcCallbackClient(lm_model=lm_model)
    scoring_case = _fetch_scoring_case(callback, run_id=payload.run, case_id=case_id)
    cx = CallbackRubricContext(
        callback,
        candidate_id=candidate,
        stage_call_id=payload.stage_call_id,
        capability_fingerprint=payload.capability_fingerprint,
        lm_model=lm_model,
    )

    value, rewards, feedback = await _evaluate_rubric(rubric, output, scoring_case, cx)
    return StageRunResult(
        schema_version="leaven.stage_run.v1",
        message="stage_run_result",
        stage="scorer",
        stage_call_id=payload.stage_call_id,
        output=OutputRecord(
            kind="text",
            summary=f"scorer feedback for {case_id}",
            value=feedback or output,
            visibility="optimizer_visible",
            data_classes=["candidate.output"],
        ),
        score=StageScore(value=value, rewards=rewards),
        effect_receipts=callback.effect_receipts(),
    )


async def _evaluate_rubric(
    rubric: Rubric,
    output: str,
    case: ScoringCaseView,
    cx: RubricContext,
) -> tuple[float, list[StageRewardFact], str]:
    """Run every reward, returning the weighted-mean score, vector, and feedback.

    The scalar value is the weight-normalized mean of the reward values, the
    same aggregation the optimizer's `score.value` consumes.
    """
    rewards: list[StageRewardFact] = []
    weighted_total = 0.0
    total_weight = 0.0
    feedback_lines: list[str] = []
    for reward in rubric.rewards:
        raw = await reward.func(output, case, cx)
        scalar, row_feedback = _normalize_reward(raw)
        rewards.append(
            StageRewardFact(
                id=reward.id,
                value=scalar,
                weight=reward.weight,
                feedback=row_feedback if row_feedback else UNSET,
            )
        )
        weighted_total += scalar * reward.weight
        total_weight += reward.weight
        if row_feedback:
            feedback_lines.append(f"{reward.id}: {row_feedback}")
    value = weighted_total / total_weight if total_weight else 0.0
    return value, rewards, "\n".join(feedback_lines)


def _normalize_reward(value: float | RewardValue) -> tuple[float, str]:
    if isinstance(value, RewardValue):
        return value.value, value.feedback
    return float(value), ""


def _fetch_scoring_case(
    callback: JsonRpcCallbackClient,
    *,
    run_id: str,
    case_id: str,
) -> ScoringCaseView:
    """Read the case target/input/metadata through gated case callbacks."""
    target = _read_case_field(callback, run_id=run_id, case_id=case_id, field="target")
    input_value = _read_case_field(callback, run_id=run_id, case_id=case_id, field="input")
    metadata = _read_case_field(callback, run_id=run_id, case_id=case_id, field="metadata")
    if input_value is None:
        raise ValueError(f"scorer case {case_id!r} has no input")
    return ScoringCaseView(
        id=case_id,
        input=_as_object(input_value, field="input"),
        metadata=_as_object(metadata, field="metadata") if metadata is not None else {},
        target=_as_object(target, field="target") if target is not None else None,
    )


def _read_case_field(
    callback: JsonRpcCallbackClient,
    *,
    run_id: str,
    case_id: str,
    field: CaseField,
) -> WireJsonLiteralDepth8 | None:
    """Send one `leaven/case.<field>` callback and return the read value."""
    request = CaseLoadRequest(
        request_id=f"worker-case-{field}-{case_id}",
        plan_id=f"plan_worker_case_{field}",
        case_id=case_id,
        include=[field],
        run_id=run_id,
    )
    primary = callback.case_read(request).primary
    match field:
        case "target":
            value = primary.target
        case "input":
            value = primary.input
        case "metadata":
            value = primary.metadata
    return None if value is UNSET else value


def _as_object(value: WireJsonLiteralDepth8, *, field: str) -> JsonObject:
    if not isinstance(value, dict):
        raise TypeError(f"scorer case {field} must be a JSON object; got {type(value).__name__}")
    return json_object(value)


def _output_text(output: OutputRecord) -> str:
    value = output.value
    if value is UNSET or not isinstance(value, str):
        raise TypeError("scorer stage output must carry a text value")
    return value


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


__all__ = ["run_scorer_stage"]
