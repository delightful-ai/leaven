"""Example 06 — custom reflect and propose functions.

When you need real Python logic in reflection/proposal (not the
engine-mediated Codex built-ins), write functions and tag them
`@lv.reflector` / `@lv.proposer`. They drop straight into the `Stages`
slots in place of `lv.Reflect.agent(...)` / `lv.Propose.agent_edit(...)`.

Build-once-pass-down: the optimizer constructs the reflective `batch` ONCE,
target-safe, and hands it to `reflect` already built. The reflector does NOT
query run history to assemble its own evidence; it reads the finished batch
(`batch.cases[i].input`, `.runs[j].output/.score/.feedback`). Reflect produces
a `Critique` (diagnosis); Propose consumes the digested `reflection` and emits
a typed `Proposal` (graph-mutation intent). They are separate stages on purpose.

`cx` is passed explicitly to both. The batch/reflection record shapes are
typed in `lv.adapters` / `lv.wire`; ordinary code reads them structurally.

Governing spec: `docs/specs/leaven_python.md` — Reflect / Propose.
"""

from __future__ import annotations

import asyncio

import leaven as lv
import leaven.adapters


@lv.runner
async def run(artifact: lv.artifacts.PromptArtifact, case: lv.Case, cx: lv.adapters.RunContext) -> str:
    return (await cx.lm.complete_text(artifact.render(**case.input))).strip()


@lv.scorer
async def correctness(run: lv.RolloutResult[str], case: lv.Case, cx: lv.adapters.RunContext) -> lv.Score:
    expected = (case.target or {})["answer"]
    return lv.Score(value=float(run.output == expected), feedback=f"got {run.output!r}")


@lv.reflector
async def reflect(batch, parent, cx: lv.adapters.RunContext) -> lv.Critique:
    """Read the pre-built, target-safe batch and diagnose failure modes.

    `feedback` is the only target-derived channel; the reflector never sees
    the raw target.
    """
    failures = [
        run.feedback
        for case in batch.cases
        for run in case.runs
        if run.score < 1.0
    ]
    return lv.Critique(
        summary=f"{len(failures)} failing runs in the minibatch",
        failure_modes=failures[:3],
        suggestions=["state the answer first", "drop the preamble"],
    )


@lv.proposer
async def propose(parent, reflection, cx: lv.adapters.RunContext) -> lv.Proposal:
    """Turn the digested reflection into a typed proposal (a prompt rewrite).

    Receives the reflector's `reflection` (summary/failure_modes/suggestions),
    NOT the raw example batch again.
    """
    addendum = " ".join(reflection.suggestions)
    return lv.Proposal(
        instructions=f"Answer the question. {addendum}\n\n{{question}}",
        rationale=reflection.summary,
    )


async def amain() -> None:
    task = lv.Task(
        cases=[
            lv.Case(id="q1", input={"question": "what is 6*7?"}, target={"answer": "42"}, split="train"),
        ],
    )
    artifact = lv.artifacts.prompt("Answer the question.\n\n{question}")

    result = await lv.evolve(
        artifact=artifact,
        task=task,
        stages=lv.Stages(rollout=run, score=correctness, reflect=reflect, propose=propose),
        optimizer=lv.optimizers.gepa(score=correctness),
        runtime=lv.runtime.local(budget=lv.budget(usd=20)),
    ).run()

    print(result.best.artifact.template)


if __name__ == "__main__":
    try:
        asyncio.run(amain())
    except NotImplementedError as e:
        print(f"(expected) {e}")
