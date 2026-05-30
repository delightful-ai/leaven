"""Example 03 — the minimal prompt-evolution program, end to end.

The smallest meaningful Leaven program: ~25 lines of user code that evolve a
prompt for an arithmetic QA task. `rollout` and `score` are functions you
write (tagged with `@lv.runner` / `@lv.scorer`); `reflect` and `propose` use
the declarative Codex-backed built-ins because this example has no custom
reflection logic. `cx` is passed explicitly to every stage fn.

`lv.evolve(...).run()` returns `Evolved[PromptArtifact]`. The stub raises at
the engine boundary; the shape is what fires taste.

Governing spec: `docs/specs/leaven_python.md` — What the user writes.
"""

from __future__ import annotations

import asyncio

import leaven as lv
import leaven.adapters


@lv.runner
async def run(artifact: lv.artifacts.PromptArtifact, case: lv.Case, cx: lv.adapters.RunContext) -> str:
    """Render the current prompt and ask the runtime LM. Returns bare `Out`;
    the engine wraps it into a `RolloutResult[str]`."""
    return (await cx.lm.complete_text(artifact.render(**case.input))).strip()


@lv.scorer
async def correctness(run: lv.RolloutResult[str], case: lv.Case, cx: lv.adapters.RunContext) -> lv.Score:
    """Exact-match scorer. The name `correctness` is what the optimizer and the
    run report refer to this score by."""
    expected = (case.target or {})["answer"]
    return lv.Score(
        value=float(run.output == expected),
        feedback=f"got {run.output!r}; expected {expected!r}",
    )


async def amain() -> None:
    task = lv.Task(
        cases=[
            lv.Case(id="q1", input={"question": "what is 6*7?"}, target={"answer": "42"}, split="train"),
            lv.Case(id="q2", input={"question": "what is 9*9?"}, target={"answer": "81"}, split="val"),
        ],
    )
    artifact = lv.artifacts.prompt(
        "Answer the question. Return only the answer.\n\nQuestion: {question}"
    )

    result = await lv.evolve(
        artifact=artifact,
        task=task,
        stages=lv.Stages(
            rollout=run,
            score=correctness,
            reflect=lv.Reflect.agent(lv.agent.codex()),
            propose=lv.Propose.agent_edit(lv.agent.codex()),
        ),
        # The optimizer references the primary score by the scorer OBJECT
        # (rename-safe; the type checker catches a typo).
        optimizer=lv.optimizers.gepa(score=correctness),
        runtime=lv.runtime.local(budget=lv.budget(usd=20)),
    ).run()

    print(result.best.artifact.template)


if __name__ == "__main__":
    try:
        asyncio.run(amain())
    except NotImplementedError as e:
        print(f"(expected) {e}")
