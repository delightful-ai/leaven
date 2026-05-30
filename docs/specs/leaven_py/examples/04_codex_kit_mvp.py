"""Example 04 — the flagship codex_kit MVP shape.

This is THE canonical agentic-evolution program: evolve Codex-shaped behavior
end to end. `rollout`, `reflect`, and `propose` are all engine-mediated Codex
declarative built-ins (no Python logic to run — the artifact IS the behavior).
The ONLY custom Python the engine calls back into is the `correctness` scorer,
which reads the completed attempt: `run.output`, `run.workspace`,
`run.sessions`, `run.status`.

Two distinct Codex roles, both load-bearing:
  - `lv.agent.codex()`         the engine-mediated EXECUTOR (runtime config).
  - `lv.artifacts.codex_kit()` the mutable behavior PACKAGE (artifact state).
The executor is fixed substrate; the behavior it executes is the artifact.

`codex_kit` requires `mutable=[...]`, validated against the known surface
(default + opt-in). `task_message.md`/`hooks.toml` are opt-in mutable.

Governing spec: `docs/specs/leaven_python.md` — codex_kit / Codex as the
default agent / What the user writes (Codex-harness example).
"""

from __future__ import annotations

import asyncio

from pydantic import BaseModel

import leaven as lv
import leaven.adapters


class Answer(BaseModel):
    answer: str


@lv.scorer
async def correctness(run: lv.RolloutResult[Answer], case: lv.Case, cx: lv.adapters.RunContext) -> lv.Score:
    """The sole custom-Python surface: judge the completed attempt. Reads the
    final workspace and the engine-mediated agent sessions off the result."""
    assert run.workspace is not None  # populated for engine-mediated rollouts
    log = await run.workspace.read_text("output/run.log", missing_ok=True)
    expected = (case.target or {})["flag"]
    return lv.Score(
        value=float(run.output.answer == expected),
        feedback=f"answer={run.output.answer!r}; sessions={len(run.sessions)}; "
        f"status={run.status}; log={log[:200]!r}",
    )


async def amain() -> None:
    task = lv.Task(
        cases=[
            lv.Case(
                id="ctf-001",
                input={"instructions": "find the flag."},
                target={"flag": "picoCTF{...}"},
                files={"challenge": lv.assets.path("assets/challenge")},
                setup=lv.setup.bash("chmod +x case/files/challenge"),
                split="train",
            ),
        ],
        sandbox=lv.sandbox.docker(image="python:3.12"),
    )

    # The mutable behavior package. `mutable=` mixes default (AGENTS.md, skills)
    # and opt-in (task_message.md, hooks.toml) surface paths.
    artifact = lv.artifacts.codex_kit(
        "./agent_kit",
        mutable=[
            "AGENTS.md",
            ".agents/skills/**/SKILL.md",
            "dev_instructions.md",
            "task_message.md",  # opt-in (case-rendered template)
            "hooks.toml",  # opt-in
        ],
    )

    # Engine-mediated rollout: the engine materializes the artifact and runs
    # Codex against it. No Python rollout body. `instructions=` is the STABLE
    # invocation envelope; the mutable instructions live in the artifact.
    rollout = lv.Rollout.agent(
        lv.agent.codex(),
        layout=lv.layouts.case_workspace(),
        instructions="Solve the case in target/current. Write output/result.json.",
        output=lv.output.json(path="output/result.json", parse_as=Answer),
    )

    result = await lv.evolve(
        artifact=artifact,
        task=task,
        stages=lv.Stages(
            rollout=rollout,
            score=correctness,
            reflect=lv.Reflect.agent(lv.agent.codex()),
            propose=lv.Propose.agent_edit(
                lv.agent.codex(),
                layout=lv.layouts.edit_artifact(),
            ),
        ),
        optimizer=lv.optimizers.gepa(
            score=correctness,
            train=lv.gepa.sampling.minibatch(split="train", size=3),
            population_size=8,
            frontier=lv.gepa.frontier.top_k(3),
        ),
        runtime=lv.runtime.local(budget=lv.budget(usd=50)),
    ).run()

    print(result.best.artifact.summary())


if __name__ == "__main__":
    try:
        asyncio.run(amain())
    except NotImplementedError as e:
        print(f"(expected) {e}")
