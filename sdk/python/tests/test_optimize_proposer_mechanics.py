from __future__ import annotations

import leaven as lv
from leaven.proposal import ProposalBatch, ProposalEffect


@lv.runner
async def run(prompt: lv.PromptArtifact, case: lv.Case, cx: lv.RolloutContext) -> str:
    _ = cx
    return prompt.template.format(**case.input)


@lv.reward(id="exact")
async def exact(output: str, case: lv.ScoringCaseView, cx: lv.RubricContext) -> float:
    _ = cx
    return 1.0 if output == (case.target or {})["answer"] else 0.0


@lv.proposer(stage_id="tests.optimize_proposer.submit_change")
async def submit_change(req, cx):
    _ = cx
    return ProposalBatch(
        effects=[
            ProposalEffect.change(
                parent_candidate_id=req.parent_candidate_id,
                surface=req.allowed_surfaces[0],
                change_schema=req.allowed_change_schemas[0],
                change={"template": "{answer}!"},
            )
        ]
    )


async def test_optimize_runs_configured_proposer_as_submit_only_slice(
    tmp_path, monkeypatch
) -> None:
    """Scenario: optimize dispatches a configured proposer and submits its batch."""

    monkeypatch.chdir(tmp_path)

    result = await lv.optimize(
        seed=lv.PromptArtifact(template="{answer}"),
        environment=lv.Environment(
            task=lv.Task(
                name="proposer-submit",
                cases=[
                    lv.Case(
                        id="submit-001",
                        input={"answer": "42"},
                        target={"answer": "42"},
                        split="train",
                    )
                ],
            ),
            rollout=lv.Rollout.fn(run),
            rubric=lv.Rubric([exact]),
        ),
        optimizer=lv.optimizers.gepa(
            population_size=1,
            propose=lv.Propose.fn(submit_change),
        ),
        runtime=lv.runtime.local(budget=lv.budget(usd=1)),
    ).run()

    assert result.summary.iterations == 1
    assert result.best.id == "cand_seed"
    assert result.frontier == [result.best]
    assert result.assessment("case_submit_001").score.value == 1.0
