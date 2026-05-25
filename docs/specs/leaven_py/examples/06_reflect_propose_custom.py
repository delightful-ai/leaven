"""Example 06 — custom reflector + proposer overriding GEPA defaults.

GEPA ships built-in reflector and proposer behaviors. When a paper needs
something different (custom diagnosis prompts, custom proposal parsing,
agentic proposers with workspace materialization), the user provides
`@lv.reflector` + `@lv.proposer` and passes them to `lv.optimize(...)`.

Reflection vs proposal are structurally separate stages by design — LMs
do one thing well, and the split is load-bearing for GEPA / ACE /
FlashEvolve patterns. See spec line: "Reflection vs proposal are
structurally separate stages."
"""

from __future__ import annotations

import asyncio
from pathlib import Path

import leaven as lv
from leaven.context import StageContext
from leaven.proposal import ProposalBatch
from leaven.stage_payloads import ProposeRequest, ReflectionResult, ReflectRequest, StageSourceRef

HERE = Path(__file__).parent
FIXTURE = HERE / "fixtures" / "arithmetic.jsonl"


@lv.reflector(stage_id="examples/custom-reflector")
async def reflect(req: ReflectRequest, cx: StageContext) -> ReflectionResult:
    # Build a diagnostic from the reflection examples. The diagnosis must
    # cite which examples it depends on via `diagnosis_source_refs`.
    failing = [ex for ex in req.examples if (ex.score or 0.0) < 0.5]
    sample = ", ".join(f"{ex.case_id}={ex.score:.2f}" for ex in failing[:5])

    diagnosis_lm = await cx.lm.complete(
        messages=[
            {
                "role": "system",
                "content": "You are a reflection agent. Diagnose why these examples failed.",
            },
            {
                "role": "user",
                "content": f"Failing examples ({len(failing)} total): {sample}",
            },
        ],
        max_tokens=512,
        model_role="reflector",
    )

    return ReflectionResult(
        diagnosis=diagnosis_lm.text,
        diagnosis_source_refs=[
            StageSourceRef(kind="reflection_example", id=ex.case_id)
            for ex in failing
        ],
        metadata={"failing_count": len(failing)},
    )


@lv.proposer(stage_id="examples/custom-proposer", repair_attempts=2)
async def propose(req: ProposeRequest, cx: StageContext) -> ProposalBatch:
    # Agentic proposer: materialize a workspace, give an agent the diagnosis,
    # let it write a typed change.
    ws = await cx.workspace.materialize_candidate(
        req.parent_candidate_id,
        surface="skills_only",
        lifetime="stage_call",
    )
    await cx.workspace.write_file(ws, "REFLECTION.md", req.reflection.diagnosis)

    session = await cx.agent.run(
        workspace=ws,
        instructions=lv.AgentInstructions(
            task="Propose a typed change to the candidate that addresses REFLECTION.md.",
            system=lv.roles.SKILL_PROPOSER,
        ),
        timeout_s=180,
    )

    # The agent's session contains the actual changes; the engine parses
    # workspace deltas into typed `ProposalEffect`s. For the scaffold,
    # `from_skill_proposal` is the convention helper that does the parse.
    return ProposalBatch.from_skill_proposal(session.parsed)


async def amain() -> None:
    pipeline = lv.optimize(
        seed=lv.SkillBank.empty(),
        train=lv.cases.from_jsonl(str(FIXTURE), name="train"),
        val=lv.cases.from_jsonl(str(FIXTURE), name="val", limit=2),
        optimizer=lv.optimizers.gepa(population_size=8),
        runtime=lv.runtime(
            workspace=lv.workspace.local(),
            lm=lv.lm.anthropic(model="claude-opus-4-7", role="reflector"),
            agent=lv.agent.codex(model="gpt-5-codex"),
            trust_profile=lv.TrustProfile.MANAGED_SANDBOX,
            budget=lv.budget(usd=150),
        ),
        # The custom reflector + proposer override GEPA's built-in defaults.
        reflector=reflect,
        proposer=propose,
    )
    print("custom reflect/propose pipeline composed.")
    print("  reflector:", pipeline.reflector)  # type: ignore[attr-defined]
    print("  proposer :", pipeline.proposer)   # type: ignore[attr-defined]


if __name__ == "__main__":
    try:
        asyncio.run(amain())
    except NotImplementedError as e:
        print(f"(expected) {e}")
