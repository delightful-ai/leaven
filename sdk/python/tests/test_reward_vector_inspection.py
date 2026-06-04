from pathlib import Path

from _pytest.monkeypatch import MonkeyPatch

import leaven as lv


@lv.runner
async def run(prompt: lv.PromptArtifact, case: lv.InputCaseView, cx: lv.RolloutContext) -> str:
    _ = cx
    return prompt.template.format(**case.input)


@lv.reward(weight=2.0, id="exact")
async def exact(output: object, case: lv.ScoringCaseView, cx: lv.RubricContext) -> float:
    _ = cx
    assert isinstance(output, str)
    return 1.0 if output == (case.target or {})["answer"] else 0.0


@lv.reward(weight=1.0, id="concise")
async def concise(
    output: object, case: lv.ScoringCaseView, cx: lv.RubricContext
) -> lv.RewardValue:
    _ = (case, cx)
    assert isinstance(output, str)
    return lv.RewardValue(value=0.5, feedback=f"{len(output)} chars")


async def test_reward_vector_aggregate_and_dimensions_survive_inspection(
    tmp_path: Path,
    monkeypatch: MonkeyPatch,
) -> None:
    """Scenario: reward vectors drive aggregate score and remain inspectable."""

    monkeypatch.chdir(tmp_path)

    result = await lv.optimize(
        seed=lv.PromptArtifact(template="{answer}"),
        environment=lv.Environment(
            task=lv.Task(
                name="reward-vector-inspection",
                cases=[
                    lv.Case(
                        id="reward-vector-001",
                        input={"answer": "42"},
                        target={"answer": "42"},
                        split="test",
                    )
                ],
            ),
            rollout=lv.Rollout.fn(run),
            rubric=lv.Rubric([exact, concise]),
        ),
        optimizer=lv.optimizers.gepa(population_size=1),
        runtime=lv.runtime.local(budget=lv.budget(usd=1)),
    ).run()

    assessment = result.assessment("case_reward_vector_001")
    assert assessment.score.value == (1.0 * 2.0 + 0.5 * 1.0) / 3.0
    assert result.best.summary_score == assessment.score.value
    assert result.summary.iterations == 0
    assert [(reward.id, reward.value, reward.weight, reward.feedback) for reward in assessment.rewards] == [
        ("exact", 1.0, 2.0, ""),
        ("concise", 0.5, 1.0, "2 chars"),
    ]

    reopened = lv.runs.open(result.summary.run_dir or "")
    reopened_assessment = reopened.assessment("case_reward_vector_001")
    assert reopened.best.summary_score == assessment.score.value
    assert [
        (reward.id, reward.value, reward.weight, reward.feedback)
        for reward in reopened_assessment.rewards
    ] == [
        ("exact", 1.0, 2.0, ""),
        ("concise", 0.5, 1.0, "2 chars"),
    ]

    inspection = lv.runs.inspect(result.summary.run_dir or "")
    assert inspection.best_candidate_id == "cand_seed"
    assert inspection.evidence[0].payload == {"output": "42", "reward_count": 2}
    assert [
        (reward.id, reward.value, reward.weight, reward.feedback)
        for reward in inspection.evidence[0].rewards
    ] == [
        ("exact", 1.0, 2.0, ""),
        ("concise", 0.5, 1.0, "2 chars"),
    ]
