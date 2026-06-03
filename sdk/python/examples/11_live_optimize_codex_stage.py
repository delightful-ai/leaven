"""Example 11 — live Codex from inside `lv.optimize(...).run()`.

This is a live-spend proof for the current Python SDK -> public seam -> Python
stage worker -> nested `leaven/agent.run` route. It is intentionally skipped by
default.

Run only when live Codex spend is intended:

    LEAVEN_LIVE_CODEX=1 uv run python examples/11_live_optimize_codex_stage.py

Set `LEAVEN_BIN` or `LEAVEN_CODEX_BIN` to override binary discovery.
"""

from __future__ import annotations

import asyncio
import os

import leaven as lv


@lv.runner
async def run(prompt: lv.PromptArtifact, case: lv.InputCaseView, cx: lv.RolloutContext) -> str:
    """Run Codex through the active stage seam and return its receipt id."""
    _ = prompt
    session = await cx.agent.run(
        workspace=cx.rollout_workspace,
        instructions=lv.AgentInstructions(
            system=(
                "You are running inside a temporary Leaven proof workspace. "
                "Do not edit files or run tools unless absolutely necessary."
            ),
            task=(
                "Return exactly this sentence as the final answer: "
                "Leaven optimize live Codex stage proof succeeded."
            ),
        ),
        output=lv.output.text(max_chars=256),
        timeout_s=120,
        input_classes=["public"],
    )
    return session.receipt.receipt_id


@lv.reward
async def exact(output: str, case: lv.ScoringCaseView, cx: lv.RubricContext) -> float:
    """Score the stage output against the target receipt."""
    _ = cx
    return 1.0 if output == (case.target or {})["answer"] else 0.0


async def amain() -> None:
    if os.environ.get("LEAVEN_LIVE_CODEX") != "1":
        print("skipped: set LEAVEN_LIVE_CODEX=1 to run the live Codex optimize proof")
        return

    result = await lv.optimize(
        seed=lv.PromptArtifact(template="Live Codex proof prompt for {question}."),
        environment=lv.Environment(
            task=lv.Task(
                name="live-codex-optimize",
                cases=[
                    lv.Case(
                        id="live-codex-001",
                        input={"question": "Can Codex run through Leaven?"},
                        target={"answer": "agentrec_completion"},
                    )
                ],
            ),
            rollout=lv.Rollout.fn(run),
            rubric=lv.Rubric([exact]),
        ),
        optimizer=lv.optimizers.gepa(population_size=1),
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

    assert result.best.summary_score == 1.0
    print("run id:          ", result.run_id)
    print("best score:      ", f"{result.best.summary_score:.3f}")
    print("cost status:     ", result.summary.cost_status)
    print("agent receipt:   ", "agentrec_completion")


if __name__ == "__main__":
    asyncio.run(amain())
