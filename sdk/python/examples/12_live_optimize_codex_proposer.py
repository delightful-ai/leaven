"""Example 12 -- live Codex from a configured proposer inside optimize().

This is a live-spend proof for the Python SDK -> public seam -> Python
proposer worker -> nested `leaven/agent.run` -> `leaven/proposal.submit_batch`
route. It is intentionally skipped by default.

Run only when live Codex spend is intended:

    LEAVEN_LIVE_CODEX=1 uv run python examples/12_live_optimize_codex_proposer.py

Set `LEAVEN_BIN` or `LEAVEN_CODEX_BIN` to override binary discovery.
"""

import asyncio
import os

import leaven as lv
from leaven.proposal import ProposalBatch, ProposalEffect
from leaven.stage_payloads import ProposeRequest


@lv.runner
async def run(prompt: lv.PromptArtifact, case: lv.InputCaseView, cx: lv.RolloutContext) -> str:
    """Run the seed prompt deterministically; proposer owns the live agent work."""
    _ = cx
    return prompt.template.format(**case.input)


@lv.reward
async def exact(output: str, case: lv.ScoringCaseView, cx: lv.RubricContext) -> float:
    """Score the seed rollout."""
    _ = cx
    return 1.0 if output == (case.target or {})["answer"] else 0.0


@lv.proposer(stage_id="examples.live_codex_proposer.submit_agent_change")
async def propose(req: ProposeRequest, cx: lv.ProposeContext) -> ProposalBatch:
    """Run Codex in the parent workspace and submit a receipt-bound proposal."""
    session = await cx.agent.run(
        workspace=cx.parent_workspace,
        instructions=lv.AgentInstructions(
            system=(
                "You are running inside a temporary Leaven proof workspace. "
                "Do not edit files or run tools unless absolutely necessary."
            ),
            task=(
                "Return exactly this sentence as the final answer: "
                "Leaven optimize live Codex proposer proof succeeded."
            ),
        ),
        output=lv.output.text(max_chars=256),
        timeout_s=120,
        input_classes=["public"],
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


async def amain() -> None:
    if os.environ.get("LEAVEN_LIVE_CODEX") != "1":
        print("skipped: set LEAVEN_LIVE_CODEX=1 to run the live Codex proposer proof")
        return

    result = await lv.optimize(
        seed=lv.PromptArtifact(template="{answer}"),
        environment=lv.Environment(
            task=lv.Task(
                name="live-codex-proposer",
                cases=[
                    lv.Case(
                        id="live-codex-proposer-001",
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
            propose=lv.Propose.fn(propose),
        ),
        runtime=lv.runtime(
            workspace=lv.workspace.local(),
            lm=lv.lm.mock(responses=["unused"]),
            agent=lv.agent.codex(
                model="gpt-5.4-mini",
                transport="cli",
                approval_mode="interactive",
                timeout_s=120,
            ),
            budget=lv.budget(usd=5),
        ),
    ).run()

    assert result.summary.iterations == 1
    assert result.best.summary_score == 1.0
    print("run id:          ", result.run_id)
    print("best score:      ", f"{result.best.summary_score:.3f}")
    print("iterations:      ", result.summary.iterations)
    print("cost status:     ", result.summary.cost_status)


if __name__ == "__main__":
    asyncio.run(amain())
