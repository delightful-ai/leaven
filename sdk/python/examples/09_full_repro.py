"""Example 09 — full front-door showcase across every product role.

The big sketch. A real user writing a paper-shaped optimization would put this
in one file (or a thin orchestration file referencing modules per stage). It
exercises every role the front door exposes on the new surface:

- the inner loop as an `Environment`: `Rollout.agent()` + a multi-reward `Rubric`
- the outer loop on the optimizer: a custom reflector and a custom proposer,
  attached via `gepa(reflect=, propose=, ...)`

Scoring here is a `Rubric` of `@lv.reward` functions — the ordinary path. The
hand-authored `@lv.evaluator` escape hatch is a separate, advanced surface; see
example 05 for that. This file currently stops at the non-PromptArtifact
front-door boundary. The point is the SHAPE firing taste, not claiming a
runnable SkillBank optimization.
"""

import asyncio
from pathlib import Path

import leaven as lv
from leaven.proposal import ProposalBatch, SkillProposal
from leaven.stage_payloads import (
    ProposeRequest,
    ReflectionFailureMode,
    ReflectionResult,
    ReflectRequest,
    StageSourceRef,
)

HERE = Path(__file__).parent
FIXTURE = HERE / "fixtures" / "arithmetic.jsonl"


# ----- inner loop: rubric — a weighted reward vector ------------------------


@lv.reward(weight=1.0)
async def correct(output: str, case: lv.ScoringCaseView, cx: lv.RubricContext) -> lv.RewardValue:
    _ = cx
    assert case.target is not None
    target = case.target["answer"]
    if not isinstance(target, str):
        raise TypeError("arithmetic fixture target answer must be text")
    return lv.RewardValue(
        value=lv.scoring.multi_tolerance(output, target),
        feedback=f"candidate answered {output!r}; target was {target!r}",
    )


@lv.reward(weight=0.3)
async def shows_work(output: str, case: lv.ScoringCaseView, cx: lv.RubricContext) -> float:
    _ = (case, cx)
    return 1.0 if "=" in output else 0.0


# ----- outer loop: reflector — typed diagnosis from failing examples --------


@lv.reflector(stage_id="examples/09-reflector")
async def reflect(req: ReflectRequest, cx: lv.ReflectContext) -> ReflectionResult:
    failing = [ex for ex in req.examples if (ex.score or 0.0) < 0.5]
    summary = ", ".join(f"{ex.case_id}={ex.score:.2f}" for ex in failing[:8])
    diagnostic = await cx.lm.complete(
        prompt=(
            "Diagnose why these examples failed.\n\n"
            f"Failing ({len(failing)}): {summary}"
        ),
        max_tokens=512,
        model_role="reflector",
    )
    return ReflectionResult(
        diagnosis=diagnostic.text,
        diagnosis_source_refs=[
            StageSourceRef(kind="reflection_example", id=ex.case_id) for ex in failing
        ],
        failure_modes=[
            ReflectionFailureMode(
                label="failing_examples",
                description=f"{len(failing)} examples failed in this reflection batch.",
                source_refs=[
                    StageSourceRef(kind="reflection_example", id=ex.case_id) for ex in failing
                ],
            )
        ],
    )


# ----- outer loop: proposer — typed change batch from a reflection ----------


@lv.proposer(stage_id="examples/09-proposer", repair_attempts=2)
async def propose(req: ProposeRequest, cx: lv.ProposeContext) -> ProposalBatch:
    ws = await cx.workspace.materialize_candidate(
        req.parent_candidate_id,
        surface="skills_only",
        lifetime="stage_call",
    )
    session = await cx.agent.run(
        workspace=ws,
        instructions=lv.AgentInstructions(
            task=(
                "Propose a typed skill-bank change addressing this "
                f"reflection:\n{req.reflection.diagnosis}"
            ),
            system=lv.roles.SKILL_PROPOSER,
        ),
        output=lv.output.json_schema(SkillProposal),
        timeout_s=180,
    )
    return ProposalBatch.from_skill_proposal(session.parsed)


# ----- composition: an EvoSkill-shaped optimization, all roles wired --------


async def amain() -> None:
    environment = lv.Environment(
        task=lv.Task(
            cases=lv.cases.from_jsonl(str(FIXTURE), limit=8).cases,
            sandbox=lv.sandbox.docker(image="python:3.12"),
        ),
        rollout=lv.Rollout.agent(),
        rubric=lv.Rubric([correct, shows_work]),
    )
    optimizer = lv.optimizers.gepa(
        population_size=10,
        frontier=lv.frontier.top_k(3),
        parent_selector="round_robin",
        reflection_lm=lv.lm.anthropic(model="claude-opus-4-7"),
        minibatch_size=4,
        objective="objective",
        reflect=lv.Reflect.fn(reflect),
        propose=lv.Propose.fn(propose),
    )
    result = await lv.optimize(
        seed=lv.SkillBank.empty(),
        environment=environment,
        optimizer=optimizer,
        runtime=lv.runtime(
            workspace=lv.workspace.local(root=".agents"),
            lm={
                "executor": lv.lm.anthropic(model="claude-opus-4-7"),
                "reflector": lv.lm.anthropic(model="claude-opus-4-7"),
                "judge": lv.lm.openai(model="gpt-5", reasoning_effort="medium"),
            },
            agent=lv.agent.codex(model="gpt-5-codex"),
            sandbox=lv.sandbox.docker(image="python:3.12"),
            trust_profile=lv.TrustProfile.MANAGED_SANDBOX,
            budget=lv.budget(usd=200, calls=2000),
        ),
    ).run()

    print(len(result.best.artifact.files), "skill files in the best bank")


if __name__ == "__main__":
    try:
        asyncio.run(amain())
    except TypeError as e:
        if not str(e).startswith("this slice optimizes a PromptArtifact seed"):
            raise
        print(f"(expected) {e}")
