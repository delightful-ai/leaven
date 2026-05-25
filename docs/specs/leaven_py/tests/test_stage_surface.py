from __future__ import annotations

import leaven as lv


def test_stage_based_evolution_surface_names_the_four_product_inputs() -> None:
    """Example test: stage composition is the canonical high-level shape."""

    task = lv.Task(
        cases=[
            lv.Case(
                id="ctf-001",
                input={"instructions": "Find the flag."},
                target={"flag": "picoCTF{...}"},
                files={"README.md": "Solve the challenge."},
                setup=lv.setup.bash("chmod +x case/files/challenge"),
            ),
        ],
        sandbox=lv.sandbox.docker(image="python:3.12"),
    )
    artifact = lv.artifacts.directory("./agent_harness")
    layout = lv.layouts.case_workspace()
    rollout = lv.Rollout.command(
        argv=["uv", "run", "python", "target/current/run.py"],
        layout=layout,
        output=lv.output.files(["output/result.json"], max_bytes=64_000),
    )
    score = lv.ScoreStage.fn(dummy_score)
    stages = lv.Stages(
        rollout=rollout,
        score=score,
        reflect=lv.Reflect.default_gepa(),
        propose=lv.Propose.agent_edit(agent=lv.agent.codex(model="gpt-5-codex")),
        evaluate=lv.Evaluate.pipeline(rollout=rollout, score=score, split="val"),
    )
    runtime = lv.runtime.local(budget=lv.budget(usd=20))

    evolution = lv.evolve(
        artifact=artifact,
        task=task,
        stages=stages,
        optimizer=lv.optimizers.gepa(population_size=4),
        runtime=runtime,
    )

    assert evolution.artifact == artifact
    assert evolution.task == task
    assert evolution.stages == stages
    assert evolution.runtime == runtime
    assert rollout.layout == layout


@lv.scorer
async def dummy_score(output: object, case: lv.Case, cx) -> lv.Score:
    """The scorer can inspect the rollout workspace prepared by the engine."""
    _workspace = cx.rollout_workspace
    return lv.Score(
        value=1.0,
        feedback="dummy score",
    )
