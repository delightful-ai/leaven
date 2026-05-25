"""Example 09 — full EvoSkill-shaped repro exercising all 6 stage roles.

The big sketch. A real user writing a paper-shaped optimization would put
this in one file (or a thin orchestration file referencing modules per
stage). Every stage role gets exercised: runner, scorer, reflector,
proposer, judge, evaluator. Plus inspection.

This file does not call `.run()` — bodies that would hit the engine raise
NotImplementedError and the example catches them. The point is the SHAPE
firing taste; not running an optimization end-to-end.

Spec: docs/specs/leaven_python.md "The Python authoring surface".
"""

from __future__ import annotations

import asyncio
from pathlib import Path

from pydantic import BaseModel, Field

import leaven as lv

HERE = Path(__file__).parent
FIXTURE = HERE / "fixtures" / "arithmetic.jsonl"


# ----- Typed JSON-schema outputs the agent + judge return ------------------


class SkillBuilderProposal(BaseModel):
    """Structured output from the skill-builder agent."""

    rationale: str
    files: list[dict[str, str]] = Field(default_factory=list)


class PairwiseJudgment(BaseModel):
    """Structured judgment between two candidates."""

    preferred_candidate: str
    confidence: float = Field(ge=0.0, le=1.0)
    reasoning: str


# ----- Stage 1: runner — execute candidate against case ---------------------


@lv.runner
async def run(bank: lv.SkillBank, case: lv.Case, cx: lv.RunContext) -> str:
    """Materialize the skill bank into a workspace, run an agent, return output."""
    ws = await cx.workspace.materialize_candidate(
        cx.candidate_id, surface="full_repo", lifetime="stage_call",
    )
    await cx.workspace.write_skills(ws, bank)
    session = await cx.agent.run(
        workspace=ws,
        instructions=lv.AgentInstructions(
            task=case.input["question"], developer=lv.roles.EXECUTOR,
        ),
        timeout_s=240,
    )
    return (session.final_message or "").strip()


# ----- Stage 2: scorer — produce a Score from output + case -----------------


@lv.scorer
async def score(output: str, case: lv.Case, cx: lv.RunContext) -> lv.Score:
    target = (case.target or {}).get("answer", "")
    return lv.Score(
        value=lv.scoring.multi_tolerance(output, target),
        output=lv.OutputRecord.text(summary=output, visibility="optimizer_visible"),
        metrics={"length_chars": float(len(output))},
    )


# ----- Stage 3: reflector — produce a typed diagnosis from failing examples -


@lv.reflector(stage_id="examples/09-reflector")
async def reflect(req: lv.ReflectRequest, cx: lv.StageContext) -> lv.ReflectionResult:
    failing = [ex for ex in req.examples if (ex.score or 0.0) < 0.5]
    summary = ", ".join(f"{ex.case_id}={ex.score:.2f}" for ex in failing[:8])
    diagnostic = await cx.lm.complete(
        messages=[
            {"role": "system", "content": "Diagnose why these examples failed."},
            {"role": "user", "content": f"Failing ({len(failing)}): {summary}"},
        ],
        max_tokens=512,
        model_role="reflector",
    )
    return lv.ReflectionResult(
        diagnosis=diagnostic.text,
        diagnosis_source_refs=[
            lv.StageSourceRef(kind="reflection_example", id=ex.case_id) for ex in failing
        ],
        metadata={"failing_count": len(failing)},
    )


# ----- Stage 4: proposer — emit a typed change batch from a reflection ------


@lv.proposer(stage_id="examples/09-proposer", repair_attempts=2)
async def propose(req: lv.ProposeRequest, cx: lv.StageContext) -> lv.ProposalBatch:
    ws = await cx.workspace.materialize_candidate(
        req.parent_candidate_id, surface="skills_only", lifetime="stage_call",
    )
    await cx.workspace.write_file(ws, "REFLECTION.md", req.reflection.diagnosis)
    session = await cx.agent.run(
        workspace=ws,
        instructions=lv.AgentInstructions(
            task="Propose a typed skill-bank change addressing REFLECTION.md.",
            developer=lv.roles.SKILL_PROPOSER,
        ),
        output=lv.output.json_schema(SkillBuilderProposal),
        timeout_s=180,
    )
    return lv.ProposalBatch.from_skill_proposal(session.parsed)


# ----- Stage 5: judge — pairwise preference between two candidates ---------


@lv.judge(stage_id="examples/09-pairwise-judge")
async def judge(req: lv.JudgeRequest, cx: lv.StageContext) -> lv.AssessmentWrite:
    case = await cx.case.load(req.case_id, include=("input", "target"))
    response = await cx.lm.complete(
        messages=[
            {"role": "system", "content": "Pairwise judge: pick the better candidate."},
            {
                "role": "user",
                "content": (
                    f"Q: {case.input['question']}\nRubric: "
                    f"{(case.target or {}).get('rubric', 'exact match')}\nCandidates: {req.candidates}"
                ),
            },
        ],
        response_format=lv.output.json_schema(PairwiseJudgment),
        model_role="judge",
    )
    parsed: PairwiseJudgment = response.parsed
    return lv.AssessmentWrite.pairwise(
        candidates=req.candidates,
        case=req.case_id,
        preference=parsed.preferred_candidate,
        evidence=lv.EvidenceEnvelope.public_only(
            payload={"reasoning": parsed.reasoning, "confidence": parsed.confidence},
            data_classes=[lv.data_class.OPTIMIZER_VISIBLE],
        ),
        read_receipts=[case.receipt],
        effect_receipts=[response.receipt],
        replayability="boundary_managed",
    )


# ----- Stage 6: evaluator — full eval loop with batched effects ------------


@lv.evaluator(
    id="examples/09-evaluator",
    trust_profile=lv.TrustProfile.MANAGED_SANDBOX,
    granularity="per_case",
)
async def evaluate(job: lv.EvaluationJob, cx: lv.EvalContext) -> lv.AssessmentSubmission:
    assessments: list[lv.AssessmentWrite] = []
    for item in job.independent_cases():
        assert item.candidate_id is not None
        case = await cx.case.load(item.case_id, include=("input", "target"))
        ws = await cx.workspace.materialize_candidate(
            item.candidate_id, surface="full_repo", lifetime="stage_call",
        )
        async with cx.batch() as b:
            diff = b.workspace.git_diff(ws, against="parent")
            tests = b.sandbox.exec(
                workspace=ws, argv=["pytest", "-q", "--json-report"], timeout_s=60,
                output=lv.output.files(["report.json"], max_bytes=64_000),
                input_classes=[lv.data_class.CASE_TARGET, lv.data_class.WORKSPACE_FILE],
                forbidden_input_classes=[lv.data_class.WORKSPACE_SECRET],
            )
        composite = 1.0 if tests.exit_code == 0 else 0.0
        assessments.append(
            lv.AssessmentWrite.independent_case(
                candidate=item.candidate_id, case=item.case_id,
                score=lv.Score(
                    value=composite,
                    output=lv.OutputRecord.text(
                        summary=f"tests {'pass' if tests.exit_code == 0 else 'fail'}",
                        visibility="optimizer_visible",
                    ),
                    metrics={"tests_exit": float(tests.exit_code)},
                ),
                evidence=lv.EvidenceEnvelope.public_private(
                    public={"tests_passed": tests.exit_code == 0,
                            "data_classes": [lv.data_class.OPTIMIZER_VISIBLE]},
                    private={"git_diff": diff.text,
                             "data_classes": [lv.data_class.CASE_TARGET, lv.data_class.EVALUATOR_PRIVATE]},
                    target_derived=True,
                ),
                read_receipts=[case.receipt, diff.receipt],
                effect_receipts=[tests.receipt],
                replayability="boundary_managed",
            ),
        )
    return await cx.assessments.submit(job.evaluation_request_id, assessments)


# ----- Composition: an EvoSkill-shaped optimization ------------------------


async def amain() -> None:
    env = lv.environment(
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
    )
    optimizer = lv.optimizers.gepa(
        population_size=10, frontier=lv.frontier.top_k(3),
        parent_selector="round_robin",
        reflection_lm=lv.lm.anthropic(model="claude-opus-4-7"),
        minibatch_size=4,
    )
    pipeline = lv.optimize(
        seed=lv.SkillBank.empty(),
        train=lv.cases.from_jsonl(str(FIXTURE), name="train", limit=6),
        val=lv.cases.from_jsonl(str(FIXTURE), name="val", limit=2),
        optimizer=optimizer, environment=env,
        # All six stage roles passed in one composition:
        runner=run, scorer=score, reflector=reflect, proposer=propose,
        judge=judge, evaluator=evaluate,
    )
    print("full repro pipeline composed with all 6 stage roles:")
    for role in ("runner", "scorer", "reflector", "proposer", "judge", "evaluator"):
        stage = getattr(pipeline, role)
        print(f"  {role:9}: {stage.id if stage else '(none)'}")
    print(f"  optimizer: {pipeline.optimizer.name} pop={pipeline.optimizer.population_size}")  # type: ignore[attr-defined]


if __name__ == "__main__":
    try:
        asyncio.run(amain())
    except NotImplementedError as e:
        print(f"(expected) {e}")
