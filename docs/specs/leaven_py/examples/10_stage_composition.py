"""Example 10 — stage-composition surface.

The canonical direction after comparing Inspect-style tasks and FlashEvolve-style
stage composition: artifact, task, swappable stages, optimizer, runtime.

The example composes only. `.run()` remains the engine boundary and raises
NotImplementedError in the scaffold.
"""

from __future__ import annotations

import asyncio

import leaven as lv


def build_task() -> lv.Task:
    """A tiny Inspect-shaped task world."""
    return lv.Task(
        cases=[
            lv.Case(
                id="arith-001",
                input={"question": "What is 2 + 3?"},
                target={"answer": "5"},
                files={"README.md": "Answer the arithmetic question."},
                setup=lv.setup.bash("mkdir -p output"),
                split="train",
            ),
        ],
        sandbox=lv.sandbox.docker(image="python:3.12"),
        name="arithmetic-harness",
    )


@lv.scorer
async def score(output: object, case: lv.Case, cx) -> lv.Score:
    """Score a command rollout.

    The runtime prepares `cx.rollout_workspace` from the rollout layout before
    invoking the runner/command/agent and keeps that handle visible while the
    scorer inspects captured files or other post-run state.
    """
    _workspace = cx.rollout_workspace
    return lv.Score(
        value=1.0,
        feedback="demo scorer accepts the command rollout output",
    )


async def amain() -> None:
    artifact = lv.artifacts.directory("./agent_harness")
    rollout = lv.Rollout.command(
        argv=["uv", "run", "python", "target/current/run.py"],
        layout=lv.layouts.case_workspace(),
        output=lv.output.files(["output/result.json"], max_bytes=64_000),
    )
    score_stage = lv.ScoreStage.fn(score)
    stages = lv.Stages(
        rollout=rollout,
        score=score_stage,
        reflect=lv.Reflect.default_gepa(),
        propose=lv.Propose.agent_edit(agent=lv.agent.codex(model="gpt-5-codex")),
        evaluate=lv.Evaluate.pipeline(rollout=rollout, score=score_stage, split="val"),
    )
    evolution = lv.evolve(
        artifact=artifact,
        task=build_task(),
        stages=stages,
        optimizer=lv.optimizers.gepa(population_size=4),
        runtime=lv.runtime.local(budget=lv.budget(usd=20)),
    )

    print("stage-composition evolution composed.")
    print("  artifact :", evolution.artifact)
    print("  rollout  :", evolution.stages.rollout.kind)
    print("  runtime  :", evolution.runtime.trust_profile)


if __name__ == "__main__":
    try:
        asyncio.run(amain())
    except NotImplementedError as e:
        print(f"(expected) {e}")
