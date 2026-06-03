from __future__ import annotations

import leaven as lv


def test_optimize_surface_names_the_four_product_inputs() -> None:
    """Example test: optimize composes seed, environment, optimizer, runtime."""

    task = lv.Task(
        name="ctf-smoke",
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
    seed = lv.artifacts.directory("./agent_harness")
    layout = lv.layouts.case_workspace()
    rollout = lv.Rollout.command(
        argv=["uv", "run", "python", "target/current/run.py"],
        layout=layout,
        output=lv.output.files(["output/result.json"], max_bytes=64_000),
    )
    rubric = lv.Rubric([dummy_reward])
    environment = lv.Environment(
        task=task,
        rollout=rollout,
        rubric=rubric,
    )
    runtime = lv.runtime.local(budget=lv.budget(usd=20))

    run = lv.optimize(
        seed=seed,
        environment=environment,
        optimizer=lv.optimizers.gepa(population_size=4),
        runtime=runtime,
    )

    assert run.seed == seed
    assert run.environment == environment
    assert run.optimizer.name == "gepa"
    assert run.runtime == runtime
    assert environment.task == task
    assert environment.rubric == rubric
    assert rollout.layout == layout


@lv.reward
async def dummy_reward(output: object, case: lv.ScoringCaseView, cx: lv.RubricContext) -> float:
    """The reward can inspect scorer-role case data."""
    _ = (output, case, cx)
    return 1.0
