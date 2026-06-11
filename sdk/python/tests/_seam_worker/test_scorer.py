"""Tests for the worker scorer's reward-vector aggregation."""

import leaven as lv
from leaven._handles import WorkspaceHandle
from leaven._receipts import CallReceipt
from leaven._seam_worker.scorer import _evaluate_rubric
from leaven.case import ScoringCaseView
from leaven.contexts import RubricContext


class _StubRubricContext(RubricContext):
    """Minimal scorer-role context; the rewards under test do not use effects."""

    @property
    def stage_id(self) -> str:
        return "sc_test_scorer"

    @property
    def capability_fingerprint(self) -> str:
        return "fp_cap_sha256_test_scorer"

    @property
    def rollout_workspace(self) -> WorkspaceHandle:
        return WorkspaceHandle(
            workspace_id="ws_test_scorer",
            candidate_id="cand_test_scorer",
            lifetime="stage_call",
            receipt=CallReceipt(receipt_id="wrec_test_scorer"),
        )


async def test_reward_vector_collapses_to_weighted_mean() -> None:
    """Law: score.value is the weight-normalized mean the optimizer selects on."""

    @lv.reward(weight=2.0, id="exact")
    async def exact(output: str, case: lv.ScoringCaseView, cx: lv.RubricContext) -> float:
        _ = (case, cx)
        return 1.0 if output == "42" else 0.0

    @lv.reward(weight=1.0, id="concise")
    async def concise(output: str, case: lv.ScoringCaseView, cx: lv.RubricContext) -> lv.RewardValue:
        _ = (case, cx)
        return lv.RewardValue(value=0.5, feedback=f"{len(output)} chars")

    rubric = lv.Rubric([exact, concise])
    case = ScoringCaseView(id="case_1", input={"q": "x"}, metadata={}, target={"answer": "42"})

    value, rewards, feedback = await _evaluate_rubric(
        rubric, "42", case, _StubRubricContext()
    )

    # Weighted mean: (1.0*2.0 + 0.5*1.0) / (2.0 + 1.0) = 2.5 / 3.0.
    assert value == (1.0 * 2.0 + 0.5 * 1.0) / 3.0
    assert [(r.id, r.value, r.weight) for r in rewards] == [
        ("exact", 1.0, 2.0),
        ("concise", 0.5, 1.0),
    ]
    # Only rewards that produced feedback render into the scorer output text.
    assert "concise" in feedback
    assert "2 chars" in feedback
