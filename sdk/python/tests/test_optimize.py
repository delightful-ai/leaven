"""Tests for `leaven.optimize` result projection."""

import leaven as lv
from leaven._seam_optimize.status import unsupported_facts_for_runtime
from leaven._seam_optimize.types import (
    PlannedOptimizeCase,
    SeamOptimizeReport,
    SeamStageAssessment,
)
from leaven.artifacts.prompt import PromptArtifact
from leaven.assessment import RewardAssessment
from leaven.optimize import _to_optimized


def test_optimize_summary_names_codex_cost_and_inspection_gaps() -> None:
    """Scenario: seam optimize summary names unsupported deps instead of zeroing them."""

    runtime = lv.runtime(
        workspace=lv.workspace.local(),
        lm=lv.lm.mock(responses=["unused"]),
        agent=lv.agent.codex(model="gpt-5.4-mini", transport="cli"),
    )
    unsupported = unsupported_facts_for_runtime(runtime)
    result = _to_optimized(
        PromptArtifact(template="answer {question}"),
        [],
        SeamOptimizeReport(seed_score=1.0, best_score=1.0, assessments=[], unsupported=unsupported),
        "codex-status-test",
    )

    assert result.summary.total_cost_usd is None
    assert result.summary.total_lm_tokens is None
    assert result.summary.cost_status == "unsupported_dependency"
    assert result.summary.usage_status == "unsupported_dependency"
    assert {
        (fact.surface, fact.dependency, fact.reason) for fact in result.summary.unsupported
    } == {
        ("run.cost", "codex_cli", "provider_cost_not_reported"),
        ("run.usage", "codex_cli", "provider_usage_not_reported"),
    }
    assert "total_cost_usd" in result.summary.unsupported[0].detail


def test_optimized_result_exposes_assessments_rewards_and_lineage() -> None:
    """Scenario: completed result inspection is not stdout-only."""

    result = _to_optimized(
        PromptArtifact(template="answer {question}"),
        [
            PlannedOptimizeCase(
                case_id="case_inspect_001",
                input={"question": "6 * 7?"},
                target={"answer": "42"},
                metadata={"source": "unit"},
                split="test",
            )
        ],
        SeamOptimizeReport(
            seed_score=0.75,
            best_score=0.75,
            assessments=[
                SeamStageAssessment(
                    case_id="case_inspect_001",
                    case_input={"question": "6 * 7?"},
                    case_target={"answer": "42"},
                    case_metadata={"source": "unit"},
                    case_split="test",
                    output="42",
                    score=lv.Score(value=0.75, feedback="weighted"),
                    rewards=[
                        RewardAssessment(id="correct", value=1.0, weight=2.0),
                        RewardAssessment(
                            id="concise",
                            value=0.25,
                            weight=1.0,
                            feedback="short enough",
                        ),
                    ],
                )
            ],
        ),
        "inspection-test",
    )

    assessment = result.assessment("case_inspect_001")

    assert result.lineage(result.best.id) == [result.best]
    assert list(result.test_assessments()) == [assessment]
    assert assessment.case.target == {"answer": "42"}
    assert assessment.score.value == 0.75
    assert [(reward.id, reward.value) for reward in assessment.rewards] == [
        ("correct", 1.0),
        ("concise", 0.25),
    ]


__all__ = []
