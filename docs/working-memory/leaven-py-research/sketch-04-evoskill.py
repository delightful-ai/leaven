"""Sketch 04 — EvoSkill-shaped repro, on the REDESIGNED surface.

Redesign sketch (2026-06-01). Same shape as sketch 03, one tier up: the
artifact is a SkillBank, the rollout is the Codex-native agent substrate, and
the outer loop (reflect/propose) lives on the optimizer. NOT yet importable
(see sketch 03 header). Compare `examples/04_evoskill_skill_bank.py` (current)
and `examples/09_full_repro.py` (the 259-line six-decorator version this
replaces).

The composition glue does not grow with the paper's complexity; only the
rollout and the rewards do.
"""

from __future__ import annotations

import asyncio

from pydantic import BaseModel

import leaven as lv


# ----- rubric: a weighted reward VECTOR (not a forced scalar) ---------------
# Two named rewards. The optimizer decides how the vector reduces
# (`objective=`); the Rubric just reports the dimensions. rubric-`cx` here
# could also reach `cx.rollout_workspace` to grade captured files.
@lv.reward(weight=1.0)
async def correct(output: str, case: lv.Case, cx) -> lv.RewardValue:
    target = (case.target or {}).get("answer", "")
    return lv.RewardValue(
        value=lv.scoring.multi_tolerance(output, target),
        feedback=f"answered {output!r}; target {target!r}",
    )


@lv.reward(weight=0.3)
async def shows_work(output: str, case: lv.Case, cx) -> float:
    return 1.0 if "=" in str(output) else 0.0


# ----- the artifact-specific proposer output --------------------------------
class SkillProposal(BaseModel):
    rationale: str
    files: list[dict[str, str]]


async def amain() -> None:
    # Environment = the inner loop. `Rollout.agent()` is the Codex-native
    # default: the engine materializes the current SkillBank into the agent's
    # workspace, the agent owns its own multi-turn loop, and `output` defaults
    # to the final agent message. No runner body to write.
    env = lv.Environment(
        task=lv.Task(
            cases=lv.cases.from_jsonl("arithmetic.jsonl", limit=8),
            sandbox=lv.sandbox.docker("python:3.12"),  # requirement; runtime provides it
        ),
        rollout=lv.Rollout.agent(),
        rubric=lv.Rubric([correct, shows_work]),
    )

    # Optimizer = the outer loop (Leaven-native, no prime-rl/verifiers analog).
    # reflect/propose/judge default to GEPA's built-ins; override `propose`
    # here to run an AGENTIC proposer on the same Codex substrate as the rollout.
    optimizer = lv.optimizers.gepa(
        population_size=10,
        frontier=lv.frontier.top_k(3),
        reflection_lm=lv.lm.anthropic("claude-opus-4-7"),
        objective="weighted",  # how the reward vector reduces for selection
        propose=lv.Propose.agent_edit(
            agent=lv.agent.codex("gpt-5-codex"),
            output=lv.output.json_schema(SkillProposal),
        ),
    )

    # seed (the mutable artifact) is passed separately, like the model-under-
    # optimization is separate from a verifiers Env.
    result = await lv.optimize(
        seed=lv.SkillBank.empty(),
        environment=env,
        optimizer=optimizer,
        runtime=lv.runtime(
            lm=lv.lm.anthropic("claude-opus-4-7"),
            agent=lv.agent.codex("gpt-5-codex"),
            trust_profile=lv.TrustProfile.MANAGED_SANDBOX,
            budget=lv.budget(usd=200, calls=2000),
        ),
    ).run()

    print(result.best.artifact.summary())


if __name__ == "__main__":
    try:
        asyncio.run(amain())
    except NotImplementedError as e:
        print(f"(expected) {e}")
