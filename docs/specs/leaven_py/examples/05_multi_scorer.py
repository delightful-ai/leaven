"""Example 05 — multiple named scorers + a primary for GEPA comparison.

The `score` slot takes one scorer OR a list of self-named scorers
(`Scorer | Sequence[Scorer]`) — Inspect's `scorer=[accuracy(), f1()]` shape:
no dict, no stringly cross-reference. Each scorer's name is its own. The
optimizer references the primary score by the scorer OBJECT
(`gepa(score=correctness)`), so a rename is caught by the type checker.

An agentic scorer (an LLM judge) is the same plain async function with
`cx.agent.run(...)` inside it — there is no separate "agentic scoring"
constructor.

Multi-objective comparison composes via `lv.gepa.compare.*`.

Governing spec: `docs/specs/leaven_python.md` — Scorer and Score / the
`score` slot.
"""

from __future__ import annotations

import asyncio

from pydantic import BaseModel

import leaven as lv
import leaven.adapters


class Answer(BaseModel):
    answer: str


class Verdict(BaseModel):
    score: float
    reason: str


@lv.scorer
async def correctness(run: lv.RolloutResult[Answer], case: lv.Case, cx: lv.adapters.RunContext) -> lv.Score:
    return lv.Score(value=float(bool(run.output.answer)), feedback="non-empty answer")


@lv.scorer
async def trajectory_quality(run: lv.RolloutResult[Answer], case: lv.Case, cx: lv.adapters.RunContext) -> lv.Score:
    return lv.Score(value=1.0 / (1 + len(run.sessions)), feedback=f"{len(run.sessions)} sessions")


@lv.scorer(name="rubric_judge")
async def judged(run: lv.RolloutResult[Answer], case: lv.Case, cx: lv.adapters.RunContext) -> lv.Score:
    """An agentic (LLM-judge) scorer: the judge sees the final environment."""
    verdict = await cx.agent.run(
        workspace=run.workspace,
        instructions=f"Grade against rubric: {(case.target or {})['rubric']}",
        output=lv.output.json(parse_as=Verdict),
    )
    assert isinstance(verdict.parsed, Verdict)  # parsed per the output= contract
    return lv.Score(value=verdict.parsed.score, feedback=verdict.parsed.reason)


async def amain() -> None:
    task = lv.Task(
        cases=[
            lv.Case(id="q1", input={"question": "explain entropy"}, target={"rubric": "..."}, split="train"),
        ],
    )
    artifact = lv.artifacts.prompt("Answer clearly.\n\n{question}")
    rollout = lv.Rollout.agent(lv.agent.codex(), output=lv.output.json(parse_as=Answer))

    result = await lv.evolve(
        artifact=artifact,
        task=task,
        stages=lv.Stages(
            rollout=rollout,
            score=[correctness, trajectory_quality, judged],
            reflect=lv.Reflect.agent(lv.agent.codex()),
            propose=lv.Propose.agent_edit(lv.agent.codex()),
        ),
        # Multi-objective: weighted comparison keyed by the scorer OBJECTS.
        optimizer=lv.optimizers.gepa(
            score=lv.gepa.compare.weighted({correctness: 0.8, trajectory_quality: 0.2}),
        ),
        runtime=lv.runtime.local(budget=lv.budget(usd=30)),
    ).run()

    print(result.best.artifact.template)


if __name__ == "__main__":
    try:
        asyncio.run(amain())
    except NotImplementedError as e:
        print(f"(expected) {e}")
