"""Tests for `leaven._seam_optimize.rewards`."""

import leaven as lv
from leaven._seam_optimize.driver import _case_input
from leaven._seam_optimize.rewards import evaluate_reward_vector
from leaven._seam_optimize.types import PlannedOptimizeCase
from leaven.artifacts.prompt import PromptArtifact


async def test_reward_vector_executes_all_registered_rewards() -> None:
    """Example: Python rubric rewards produce per-axis rows and aggregate score."""

    @lv.reward(weight=2.0, id="correct")
    async def correct(output: str, case: lv.ScoringCaseView, cx: lv.RubricContext) -> float:
        _ = cx
        assert case.target is not None
        return 1.0 if output == (case.target or {})["answer"] else 0.0

    @lv.reward(weight=1.0, id="concise")
    async def concise(
        output: str, case: lv.ScoringCaseView, cx: lv.RubricContext
    ) -> lv.RewardValue:
        _ = (case, cx)
        return lv.RewardValue(value=0.5, feedback=f"{len(output)} chars")

    score, rewards = await evaluate_reward_vector(
        rubric=lv.Rubric([correct, concise]),
        output="42",
        case=PlannedOptimizeCase(
            case_id="case_reward_vector",
            input={"question": "6 * 7?"},
            target={"answer": "42"},
            metadata={"source": "unit"},
            split=None,
        ),
    )

    assert score.value == 0.8333333333333334
    assert score.feedback == "concise: 2 chars"
    assert [(reward.id, reward.value, reward.weight) for reward in rewards] == [
        ("correct", 1.0, 2.0),
        ("concise", 0.5, 1.0),
    ]


def test_optimize_case_input_preserves_nested_stage_run_values() -> None:
    """Regression: planned cases keep nested JSON inside runner case_input."""

    case = PlannedOptimizeCase(
        case_id="case_nested_input",
        input={"question": "2 + 2", "nested": {"answer": "4"}},
        target=None,
        metadata={},
        split=None,
    )

    assert _case_input(PromptArtifact(template="{question}"), case) == {
        "question": "2 + 2",
        "nested": {"answer": "4"},
        "prompt": "2 + 2",
    }


__all__ = []
