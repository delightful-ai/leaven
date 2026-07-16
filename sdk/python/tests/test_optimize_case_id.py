"""Case identities remain unambiguous when optimization crosses the wire."""

import pytest

import leaven as lv
from leaven.optimize import OptimizeBuilder


@lv.runner
async def _run(
    prompt: lv.PromptArtifact,
    case: lv.InputCaseView,
    cx: lv.RolloutContext,
) -> str:
    _ = (prompt, case, cx)
    return ""


@lv.reward
async def _reward(
    output: str,
    case: lv.ScoringCaseView,
    cx: lv.RubricContext,
) -> float:
    _ = (output, case, cx)
    return 0.0


def _builder(*case_ids: str) -> OptimizeBuilder[lv.PromptArtifact]:
    builder = OptimizeBuilder[lv.PromptArtifact]()
    builder.environment = lv.Environment(
        task=lv.Task(
            cases=[
                lv.Case(id=case_id, input={"id": case_id}, target={"id": case_id})
                for case_id in case_ids
            ]
        ),
        rollout=lv.Rollout.fn(_run),
        rubric=lv.Rubric([_reward]),
    )
    return builder


def test_plan_cases_preserves_hyphen_and_underscore_identities() -> None:
    """Law: distinct valid source ids remain distinct wire ids."""
    planned = _builder("item-1", "item_1")._plan_cases()

    assert [case.case_id for case in planned] == ["case_item-1", "case_item_1"]


def test_plan_cases_refuses_projected_case_id_collisions() -> None:
    """Regression: a scorer must never receive two cases under one wire id."""
    with pytest.raises(ValueError, match=r"duplicate: 'case_item'"):
        _builder("item", "case_item")._plan_cases()
