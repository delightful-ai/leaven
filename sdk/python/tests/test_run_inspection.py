from pathlib import Path

from _pytest.monkeypatch import MonkeyPatch

import leaven as lv


@lv.runner
async def run(prompt: lv.PromptArtifact, case: lv.InputCaseView, cx: lv.RolloutContext) -> str:
    _ = cx
    return prompt.template.format(**case.input)


@lv.reward(id="exact")
async def exact(output: object, case: lv.ScoringCaseView, cx: lv.RubricContext) -> float:
    _ = cx
    assert isinstance(output, str)
    return 1.0 if output == (case.target or {})["answer"] else 0.0


@lv.reward(weight=0.25, id="short")
async def short(output: object, case: lv.ScoringCaseView, cx: lv.RubricContext) -> lv.RewardValue:
    _ = (case, cx)
    assert isinstance(output, str)
    return lv.RewardValue(value=1.0 if len(output) < 8 else 0.0, feedback=f"{len(output)} chars")


async def test_optimize_persists_openable_inspection_result(
    tmp_path: Path,
    monkeypatch: MonkeyPatch,
) -> None:
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
    inspection = lv.runs.inspect(result.summary.run_dir or "")
    assert inspection.run_id == result.run_id
    assert inspection.run_dir == result.summary.run_dir
    assert inspection.best_candidate_id == "cand_seed"
    assert inspection.best_lineage == ["cand_seed"]
    assert inspection.total_cost_usd == 0.0
    assert inspection.cost_status == "known"
    assert inspection.total_lm_tokens == 0
    assert inspection.usage_status == "known"
    assert [fact.surface for fact in inspection.unsupported] == ["run.inspection"]
    assert inspection.receipt_ids(kind="write") == ["assessmentrec_case_persist_001_1"]
    assert inspection.evidence[0].case_id == "case_persist_001"
    assert inspection.evidence[0].candidate_id == "cand_seed"
    assert inspection.evidence[0].data_classes == ["public"]
    assert inspection.evidence[0].payload == {"output": "42", "reward_count": 2}
