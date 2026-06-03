from __future__ import annotations

import leaven as lv


@lv.runner
async def run(prompt: lv.PromptArtifact, case: lv.Case, cx: lv.RolloutContext) -> str:
    _ = cx
    return prompt.template.format(**case.input)


@lv.reward(id="exact")
async def exact(output: str, case: lv.ScoringCaseView, cx: lv.RubricContext) -> float:
    _ = cx
    return 1.0 if output == (case.target or {})["answer"] else 0.0


@lv.reward(weight=0.25, id="short")
async def short(output: str, case: lv.ScoringCaseView, cx: lv.RubricContext) -> lv.RewardValue:
    _ = (case, cx)
    return lv.RewardValue(value=1.0 if len(output) < 8 else 0.0, feedback=f"{len(output)} chars")


async def test_optimize_persists_openable_inspection_result(tmp_path, monkeypatch) -> None:
    """Scenario: a completed SDK run can be reopened with scores, rewards, and lineage."""

    monkeypatch.chdir(tmp_path)
    result = await lv.optimize(
        seed=lv.PromptArtifact(template="{answer}"),
        environment=lv.Environment(
            task=lv.Task(
                name="inspection-persist",
                cases=[
                    lv.Case(
                        id="persist-001",
                        input={"answer": "42"},
                        target={"answer": "42"},
                        split="test",
                    )
                ],
            ),
            rollout=lv.Rollout.fn(run),
            rubric=lv.Rubric([exact, short]),
        ),
        optimizer=lv.optimizers.gepa(population_size=1),
        runtime=lv.runtime.local(budget=lv.budget(usd=1)),
    ).run()

    reopened = lv.runs.open(result.summary.run_dir or "")

    assert result.summary.run_dir == ".leaven/runs/inspection_persist"
    assert lv.runs.list_local() == ["inspection_persist"]
    assert reopened.run_id == result.run_id
    assert reopened.best.artifact == lv.PromptArtifact(
        template="{answer}", candidate_id="cand_seed"
    )
    assert reopened.lineage(reopened.best.id) == [reopened.best]
    assessment = reopened.assessment("case_persist_001")
    assert assessment.case.target == {"answer": "42"}
    assert assessment.score.value == 1.0
    assert [(reward.id, reward.value, reward.weight) for reward in assessment.rewards] == [
        ("exact", 1.0, 1.0),
        ("short", 1.0, 0.25),
    ]
    assert reopened.summary.cost_status == "known"
    assert reopened.summary.unsupported
