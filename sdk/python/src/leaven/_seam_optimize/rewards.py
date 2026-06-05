"""Reward-vector execution for the durable-seam optimize mechanics path."""

from .._handles import WorkspaceHandle
from .._receipts import CallReceipt
from ..assessment import RewardAssessment
from ..case import ScoringCaseView
from ..contexts import RubricContext
from ..output_record import OutputRecord
from ..rubric import RewardValue, Rubric
from ..score import Score
from .types import PlannedOptimizeCase


async def evaluate_reward_vector(
    *,
    rubric: Rubric,
    output: str,
    case: PlannedOptimizeCase,
) -> tuple[Score, list[RewardAssessment]]:
    """Run every configured Python reward and return aggregate + vector rows."""
    if not rubric.rewards:
        raise ValueError("rubric must contain at least one reward")

    scoring_case = ScoringCaseView(
        id=case.case_id,
        input=dict(case.input),
        metadata=dict(case.metadata),
        target=dict(case.target) if case.target is not None else None,
    )
    rows = []
    weighted_total = 0.0
    total_weight = 0.0
    feedback = []
    for reward in rubric.rewards:
        value = await reward.func(output, scoring_case, _RubricContext())
        scalar, row_feedback, row_output = _normalize_reward_value(value)
        rows.append(
            RewardAssessment(
                id=reward.id,
                value=scalar,
                weight=reward.weight,
                feedback=row_feedback,
                output=row_output,
            )
        )
        weighted_total += scalar * reward.weight
        total_weight += reward.weight
        if row_feedback:
            feedback.append(f"{reward.id}: {row_feedback}")

    aggregate = weighted_total / total_weight if total_weight else 0.0
    return Score(value=aggregate, feedback="\n".join(feedback)), rows


class _RubricContext(RubricContext):
    """Minimal scorer-role context for pure Python reward mechanics."""

    @property
    def stage_id(self) -> str:
        return "sc_python_reward_vector"

    @property
    def capability_fingerprint(self) -> str:
        return "fp_cap_sha256_python_reward_vector"

    @property
    def rollout_workspace(self) -> WorkspaceHandle:
        return WorkspaceHandle(
            workspace_id="ws_python_reward_vector",
            candidate_id="cand_seed",
            lifetime="stage_call",
            receipt=CallReceipt(receipt_id="wrec_python_reward_vector"),
        )


def _normalize_reward_value(value: float | RewardValue) -> tuple[float, str, OutputRecord | None]:
    if isinstance(value, RewardValue):
        return value.value, value.feedback, value.output
    return float(value), "", None


__all__ = ["evaluate_reward_vector"]
