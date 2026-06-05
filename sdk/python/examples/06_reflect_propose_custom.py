"""Example 06 — custom reflector + proposer overriding GEPA defaults.

GEPA ships built-in reflector and proposer behaviors. When a paper needs
something different (custom diagnosis prompts, custom proposal parsing,
agentic proposers with workspace materialization), the user provides
`@lv.reflector` + `@lv.proposer` and attaches them to the optimizer via
`gepa(reflect=lv.Reflect.fn(...), propose=lv.Propose.fn(...))`.

Reflection vs proposal are structurally separate stages by design — LMs do one
thing well, and the split is load-bearing for GEPA / ACE / FlashEvolve patterns.
The two contexts differ in capability: a `ReflectContext` is target-safe and has
no workspace materialization; a `ProposeContext` may materialize the parent
candidate, write into its workspace, and submit a typed proposal.
"""

import asyncio
from pathlib import Path

import leaven as lv
from leaven.proposal import ProposalBatch, SkillProposal
from leaven.stage_payloads import ProposeRequest, ReflectionResult, ReflectRequest, StageSourceRef

HERE = Path(__file__).parent
FIXTURE = HERE / "fixtures" / "arithmetic.jsonl"


# ----- rollout + rubric: the inner loop the optimizer wraps -----------------


@lv.runner
async def run(bank: lv.SkillBank, case: lv.InputCaseView, cx: lv.RolloutContext) -> str:
    _ = bank
    question = case.input["question"]
    if not isinstance(question, str):
        raise TypeError("arithmetic fixture question must be text")
    reply = await cx.lm.complete(prompt=question, max_tokens=64)
    return reply.text.strip()


@lv.reward
async def exact(output: str, case: lv.ScoringCaseView, cx: lv.RubricContext) -> float:
    _ = cx
    assert case.target is not None
    return 1.0 if output == case.target["answer"] else 0.0


# ----- custom reflector: target-safe diagnosis over reflection examples -----
# A `ReflectContext` has `cx.lm` but no candidate materialization; the
# reflective dataset arrives via the request payload, never `cx.case`.
@lv.reflector(stage_id="examples/custom-reflector")
async def reflect(req: ReflectRequest, cx: lv.ReflectContext) -> ReflectionResult:
    failing = [ex for ex in req.examples if (ex.score or 0.0) < 0.5]
    sample = ", ".join(f"{ex.case_id}={ex.score:.2f}" for ex in failing[:5])

    diagnosis = await cx.lm.complete(
        messages=[
            {
                "role": "system",
                "content": "You are a reflection agent. Diagnose why these examples failed.",
            },
            {"role": "user", "content": f"Failing examples ({len(failing)} total): {sample}"},
        ],
        max_tokens=512,
        model_role="reflector",
    )

    # The diagnosis must cite which examples it depends on via source refs.
    return ReflectionResult(
        diagnosis=diagnosis.text,
        diagnosis_source_refs=[
            StageSourceRef(kind="reflection_example", id=ex.case_id) for ex in failing
        ],
        metadata={"failing_count": len(failing)},
    )


# ----- custom proposer: agentic, materializes a workspace and submits change --
# A `ProposeContext` may materialize the parent candidate and run an agent
# against it. Workspace mutation is owned by the agent/proposal path, not a
# standalone Python `workspace.write_file` helper in V1.
@lv.proposer(stage_id="examples/custom-proposer", repair_attempts=2)
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
                "Propose a typed change to the candidate that addresses this "
                f"reflection:\n{req.reflection.diagnosis}"
            ),
            system=lv.roles.SKILL_PROPOSER,
        ),
        output=lv.output.json_schema(SkillProposal),
        timeout_s=180,
    )

    # The agent's structured output is the public skill-proposal record. The
    # helper lowers it into typed `ProposalEffect`s.
    return ProposalBatch.from_skill_proposal(session.parsed)


# ----- composition: attach the custom stages to GEPA's outer loop -----------
async def amain() -> None:
    result = await lv.optimize(
        seed=lv.SkillBank.empty(),
        environment=lv.Environment(
            task=lv.Task(cases=lv.cases.from_jsonl(str(FIXTURE), limit=8).cases),
            rollout=lv.Rollout.fn(run),
            rubric=lv.Rubric([exact]),
        ),
        optimizer=lv.optimizers.gepa(
            population_size=8,
            # The custom reflector + proposer override GEPA's built-in defaults.
            reflect=lv.Reflect.fn(reflect),
            propose=lv.Propose.fn(propose),
        ),
        runtime=lv.runtime(
            workspace=lv.workspace.local(),
            lm=lv.lm.anthropic(model="claude-opus-4-7", role="reflector"),
            agent=lv.agent.codex(model="gpt-5-codex"),
            trust_profile=lv.TrustProfile.MANAGED_SANDBOX,
            budget=lv.budget(usd=150),
        ),
    ).run()

    print(len(result.best.artifact.files), "skill files after custom reflect/propose")


if __name__ == "__main__":
    try:
        asyncio.run(amain())
    except TypeError as e:
        if not str(e).startswith("this slice optimizes a PromptArtifact seed"):
            raise
        print(f"(expected) {e}")
