from pathlib import Path

from _pytest.monkeypatch import MonkeyPatch

import leaven as lv


@lv.runner
async def run_with_lm(
    prompt: lv.PromptArtifact,
    case: lv.InputCaseView,
    cx: lv.RolloutContext,
) -> str:
    reply = await cx.lm.complete(prompt=prompt.template.format(**case.input), max_tokens=12)
    return reply.text


@lv.runner
async def run_with_agent(
    prompt: lv.PromptArtifact,
    case: lv.InputCaseView,
    cx: lv.RolloutContext,
) -> str:
    _ = prompt
    session = await cx.agent.run(
        workspace=cx.rollout_workspace,
        instructions=lv.AgentInstructions(task=f"answer {case.input['question']}"),
        output=lv.output.text(max_chars=128),
    )
    return session.transcript_ref


@lv.reward(id="exact")
async def exact(output: str, case: lv.ScoringCaseView, cx: lv.RubricContext) -> float:
    _ = cx
    assert case.target is not None
    return 1.0 if output == (case.target or {})["answer"] else 0.0


async def test_run_inspection_preserves_callback_effect_receipts(
    tmp_path: Path,
    monkeypatch: MonkeyPatch,
) -> None:
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

    run_dir = result.summary.run_dir
    assert run_dir is not None

    reopened = lv.runs.open(run_dir)

    assessment = reopened.assessment("case_receipt_001")
    assert [receipt.receipt_id for receipt in assessment.effect_receipts] == ["lmrec_completion"]
    assert reopened.summary.total_lm_tokens == 2
    inspection = lv.runs.inspect(run_dir)
    assert inspection.receipt_ids(kind="call") == ["lmrec_completion"]
    assert "lmrec_completion" in inspection.receipt_ids()
    assert inspection.total_lm_tokens == 2


async def test_run_inspection_preserves_agent_transcript_blob_refs_from_rust_evidence(
    tmp_path: Path,
    monkeypatch: MonkeyPatch,
) -> None:
    """Scenario: Rust-owned assessment evidence keeps callback transcript refs."""

    monkeypatch.chdir(tmp_path)
    codex_bin = tmp_path / "fake-codex"
    codex_bin.write_text(
        """#!/bin/sh
last=""
while [ "$#" -gt 0 ]; do
  if [ "$1" = "--output-last-message" ]; then
    shift
    last="$1"
  fi
  shift || true
done
mkdir -p "$(dirname "$last")"
printf 'fake codex runner final\\n' > "$last"
printf '{"type":"message","content":"ok"}\\n'
""",
        encoding="utf-8",
    )
    codex_bin.chmod(0o755)
    monkeypatch.setenv("LEAVEN_TEST_CODEX_BIN", str(codex_bin))

    result = await lv.optimize(
        seed=lv.PromptArtifact(template="{question}"),
        environment=lv.Environment(
            task=lv.Task(
                name="agent-receipt-inspection",
                cases=[
                    lv.Case(
                        id="agent-receipt-001",
                        input={"question": "return transcript ref"},
                        target={"answer": "blob_completion_transcript"},
                        split="test",
                    )
                ],
            ),
            rollout=lv.Rollout.fn(run_with_agent),
            rubric=lv.Rubric([exact]),
        ),
        optimizer=lv.optimizers.gepa(population_size=1),
        runtime=lv.runtime(
            workspace=lv.workspace.local(),
            lm=lv.lm.mock(responses=["unused"]),
            agent=lv.agent.codex(
                model="gpt-5.4-mini",
                transport="cli",
                bin_path_env="LEAVEN_TEST_CODEX_BIN",
                approval_mode="interactive",
                timeout_s=60,
            ),
            trust_profile=lv.TrustProfile.TRUSTED_LOCAL_OPERATOR,
            budget=lv.budget(usd=1),
        ),
    ).run()

    reopened = lv.runs.open(result.summary.run_dir or "")
    reopened_assessment = reopened.assessment("case_agent_receipt_001")
    assert reopened_assessment.score.value == 1.0
    assert reopened_assessment.effect_receipts[0].receipt_id == "agentrec_completion"
    assert reopened_assessment.effect_receipts[0].blob_refs[0].blob_id == (
        "blob_completion_transcript"
    )
    assert reopened_assessment.effect_receipts[0].blob_refs[0].data_classes == [
        "transcript.raw"
    ]

    inspection = lv.runs.inspect(result.summary.run_dir or "")
    call_receipts = [receipt for receipt in inspection.receipts if receipt.kind == "call"]
    assert [receipt.receipt_id for receipt in call_receipts] == ["agentrec_completion"]
    assert call_receipts[0].blob_refs[0].blob_id == "blob_completion_transcript"
    assert call_receipts[0].blob_refs[0].data_classes == ["transcript.raw"]
    assert len(inspection.rust_stage_journal_blobs) == 1
    transcript_bytes = inspection.rust_stage_journal_blobs[0].content_bytes()
    assert b"answer return transcript ref" in transcript_bytes
    assert b"fake codex runner final" in transcript_bytes
