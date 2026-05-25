"""Example 04 — EvoSkill-shaped repro at scale.

The canonical big sketch: GEPA optimizing a SkillBank over OfficeQA-shaped
cases, using a real LM for the runner + a real agent in a materialized
workspace. ~80 lines of user code replaces ~2,400 lines of Rust glue per
the EvoSkill survey.

Note: this composes against a benchmark-shaped fixture; in production the
user would `lv.cases.from_parquet(...)` against a real OfficeQA split or
`from leaven_benchmarks_officeqa import officeqa` once that catalog ships.
"""

from __future__ import annotations

import asyncio
from pathlib import Path

import leaven as lv

HERE = Path(__file__).parent
FIXTURE = HERE / "fixtures" / "arithmetic.jsonl"


# ---- Stages ---------------------------------------------------------------


@lv.runner
async def run(
    bank: lv.SkillBank,
    case: lv.Case,
    cx,
) -> str:
    ws = await cx.workspace.materialize_candidate(
        cx.candidate_id,
        surface="full_repo",
        lifetime="stage_call",
    )
    await cx.workspace.write_skills(ws, bank)

    session = await cx.agent.run(
        workspace=ws,
        instructions=lv.AgentInstructions(
            task=case.input["question"],
        ),
        timeout_s=240,
    )
    return (session.final_message or "").strip()


@lv.scorer
async def score(output: str, case: lv.Case, cx) -> lv.Score:
    target = (case.target or {}).get("answer", "")
    return lv.Score(
        value=lv.scoring.multi_tolerance(output, target),
        feedback=f"candidate answered {output!r}; target was {target!r}",
    )


# ---- Composition ----------------------------------------------------------


def build_runtime() -> lv.Runtime:
    return lv.runtime(
        workspace=lv.workspace.local(root=".agents"),
        lm=lv.lm.anthropic(model="claude-opus-4-7"),
        agent=lv.agent.codex(model="gpt-5-codex"),
        trust_profile=lv.TrustProfile.MANAGED_SANDBOX,
        budget=lv.budget(usd=200, calls=2000),
    )


def build_optimizer() -> lv.optimizers.Gepa:
    return lv.optimizers.gepa(
        population_size=10,
        frontier=lv.frontier.top_k(3),
        parent_selector="round_robin",
        reflection_lm=lv.lm.anthropic(model="claude-opus-4-7"),
        minibatch_size=4,
    )


async def amain() -> None:
    pipeline = lv.optimize(
        seed=lv.SkillBank.empty(),
        train=lv.cases.from_jsonl(str(FIXTURE), name="train", limit=6),
        val=lv.cases.from_jsonl(str(FIXTURE), name="val", limit=2),
        optimizer=build_optimizer(),
        runtime=build_runtime(),
        runner=run,
        scorer=score,
    )
    print("EvoSkill-shaped pipeline composed.")
    print("  population_size:", pipeline.optimizer.population_size)  # type: ignore[attr-defined]
    print("  frontier       :", pipeline.optimizer.frontier)         # type: ignore[attr-defined]


if __name__ == "__main__":
    try:
        asyncio.run(amain())
    except NotImplementedError as e:
        print(f"(expected) {e}")
