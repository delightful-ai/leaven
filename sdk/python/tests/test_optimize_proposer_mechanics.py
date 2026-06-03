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


@lv.proposer(stage_id="tests.optimize_proposer.submit_agent_change")
async def submit_agent_change(req, cx):
    session = await cx.agent.run(
        workspace=cx.parent_workspace,
        instructions=lv.AgentInstructions(task="propose a prompt edit"),
        output=lv.output.text(max_chars=128),
    )
    return ProposalBatch(
        effects=[
            ProposalEffect.change_from_agent_session(
                parent_candidate_id=req.parent_candidate_id,
                surface=req.allowed_surfaces[0],
                change_schema=req.allowed_change_schemas[0],
                parser="leaven.agent_session.prompt_patch.v1",
                agent_session_receipt=session.receipt,
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
    assert [receipt.receipt_id for receipt in result.proposal_receipts] == ["wrec_proposal_batch"]
    assert result.proposal_receipts[0].proposal_ids == ["prop_proposal_batch_0"]
    assert result.best.id == "cand_seed"
    assert result.frontier == [result.best]
    assert result.assessment("case_submit_001").score.value == 1.0

    reopened = lv.runs.open(result.summary.run_dir or "")
    assert [receipt.receipt_id for receipt in reopened.proposal_receipts] == ["wrec_proposal_batch"]
    assert reopened.proposal_receipts[0].proposal_ids == ["prop_proposal_batch_0"]

    inspection = lv.runs.inspect(result.summary.run_dir or "")
    proposal_receipt = next(
        receipt for receipt in inspection.receipts if receipt.receipt_id == "wrec_proposal_batch"
    )
    assert proposal_receipt.kind == "write"
    assert proposal_receipt.source == "proposal_batch"
    assert proposal_receipt.proposal_ids == ["prop_proposal_batch_0"]


async def test_optimize_proposer_can_run_agent_then_submit_agent_session_change(
    tmp_path, monkeypatch
) -> None:
    """Scenario: configured proposer can cite a seam-agent receipt in its proposal."""

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
printf 'fake codex proposer final\\n' > "$last"
printf '{"type":"message","content":"ok"}\\n'
""",
        encoding="utf-8",
    )
    codex_bin.chmod(0o755)
    monkeypatch.setenv("LEAVEN_TEST_CODEX_BIN", str(codex_bin))

    result = await lv.optimize(
        seed=lv.PromptArtifact(template="{answer}"),
        environment=lv.Environment(
            task=lv.Task(
                name="proposer-agent-submit",
                cases=[
                    lv.Case(
                        id="agent-submit-001",
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
            propose=lv.Propose.fn(submit_agent_change),
        ),
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
            budget=lv.budget(usd=1),
        ),
    ).run()

    assert result.summary.iterations == 1
    assert result.best.id == "cand_seed"
    assert result.assessment("case_agent_submit_001").score.value == 1.0
    assert [receipt.receipt_id for receipt in result.effect_receipts] == [
        "agentrec_completion"
    ]
    transcript = result.effect_receipts[0].blob_refs[0]
    assert transcript.blob_id == "blob_completion_transcript"
    assert transcript.data_classes == ["transcript.raw"]

    reopened = lv.runs.open(result.summary.run_dir or "")
    assert reopened.effect_receipts[0].blob_refs[0].blob_id == "blob_completion_transcript"

    inspection = lv.runs.inspect(result.summary.run_dir or "")
    agent_receipt = next(
        receipt for receipt in inspection.receipts if receipt.receipt_id == "agentrec_completion"
    )
    assert agent_receipt.source == "proposer_stage"
    assert agent_receipt.blob_refs[0].blob_id == "blob_completion_transcript"
