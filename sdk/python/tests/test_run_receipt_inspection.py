from __future__ import annotations

import leaven as lv


@lv.runner
async def run_with_lm(
    prompt: lv.PromptArtifact,
    case: lv.Case,
    cx: lv.RolloutContext,
) -> str:
    reply = await cx.lm.complete(prompt=prompt.template.format(**case.input), max_tokens=12)
    return reply.text


@lv.reward(id="exact")
async def exact(output: str, case: lv.ScoringCaseView, cx: lv.RubricContext) -> float:
    _ = cx
    return 1.0 if output == (case.target or {})["answer"] else 0.0


async def test_run_inspection_preserves_callback_effect_receipts(tmp_path, monkeypatch) -> None:
    """Scenario: callback receipts survive persisted run inspection."""

    monkeypatch.chdir(tmp_path)
    result = await lv.optimize(
        seed=lv.PromptArtifact(template="{question}"),
        environment=lv.Environment(
            task=lv.Task(
                name="receipt-inspection",
                cases=[
                    lv.Case(
                        id="receipt-001",
                        input={"question": "say receipt-ok"},
                        target={"answer": "receipt-ok"},
                        split="test",
                    )
                ],
            ),
            rollout=lv.Rollout.fn(run_with_lm),
            rubric=lv.Rubric([exact]),
        ),
        optimizer=lv.optimizers.gepa(population_size=1),
        runtime=lv.runtime(
            workspace=lv.workspace.local(),
            lm=lv.lm.mock(responses=["receipt-ok"]),
            trust_profile=lv.TrustProfile.TRUSTED_LOCAL_OPERATOR,
            budget=lv.budget(usd=1),
        ),
    ).run()

    reopened = lv.runs.open(result.summary.run_dir or "")

    assessment = reopened.assessment("case_receipt_001")
    assert [receipt.receipt_id for receipt in assessment.effect_receipts] == ["lmrec_completion"]
    assert reopened.summary.total_lm_tokens == 2
    inspection = lv.runs.inspect(result.summary.run_dir or "")
    assert inspection.receipt_ids(kind="call") == ["lmrec_completion"]
    assert "lmrec_completion" in inspection.receipt_ids()
    assert inspection.total_lm_tokens == 2
