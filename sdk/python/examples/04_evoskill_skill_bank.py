"""Example 04 — EvoSkill-shaped repro, one tier up from the minimal sketch.

Same shape as example 03 (`seed x Environment x optimizer x runtime`), but the
artifact is a `SkillBank`, the rollout is the Codex-native agent substrate, and
the outer loop (reflect/propose) lives on the optimizer.

`Rollout.agent()` is the Codex-native default: the engine materializes the
current SkillBank into the agent's workspace, the agent owns its own multi-turn
loop, and `output` defaults to the final agent message. There is no runner body
to write. The composition glue does not grow with the paper's complexity — only
the rollout and the rewards do.
"""

import asyncio
from pathlib import Path

import leaven as lv

HERE = Path(__file__).parent
FIXTURE = HERE / "fixtures" / "arithmetic.jsonl"


# ----- rubric: a weighted reward VECTOR (not a forced scalar) ---------------
# Two named rewards. The optimizer decides how the vector reduces for selection
# (`objective=`); the Rubric just reports the dimensions. The rubric `cx` could
# also reach `cx.rollout_workspace` to grade captured files.
@lv.reward(weight=1.0)
async def correct(output: str, case: lv.ScoringCaseView, cx: lv.RubricContext) -> lv.RewardValue:
    _ = cx
    target = (case.target or {}).get("answer", "")
    return lv.RewardValue(
        value=lv.scoring.multi_tolerance(output, target),
        feedback=f"answered {output!r}; target {target!r}",
    )


@lv.reward(weight=0.3)
async def shows_work(output: str, case: lv.ScoringCaseView, cx: lv.RubricContext) -> float:
    _ = (case, cx)
    return 1.0 if "=" in str(output) else 0.0


async def amain() -> None:
    # Environment = the inner loop. `Rollout.agent()` runs the runtime's agent
    # against each case with no hand-written runner.
    env = lv.Environment(
        task=lv.Task(
            cases=lv.cases.from_jsonl(str(FIXTURE), limit=8).cases,
            sandbox=lv.sandbox.docker(image="python:3.12"),  # requirement; runtime provides it
        ),
        rollout=lv.Rollout.agent(),
        rubric=lv.Rubric([correct, shows_work]),
    )

    # Optimizer = the outer loop. reflect/propose default to GEPA's built-ins;
    # override `propose` here to run an AGENTIC proposer on the same Codex
    # substrate as the rollout. `objective="objective"` keeps a per-reward-
    # dimension Pareto frontier over the rubric's reward vector.
    optimizer = lv.optimizers.gepa(
        population_size=10,
        frontier=lv.frontier.top_k(3),
        reflection_lm=lv.lm.anthropic(model="claude-opus-4-7"),
        objective="objective",
        propose=lv.Propose.agent_edit(agent=lv.agent.codex(model="gpt-5-codex")),
    )

    # The seed (the mutable artifact) is passed separately from the environment.
    result = await lv.optimize(
        seed=lv.SkillBank.empty(),
        environment=env,
        optimizer=optimizer,
        runtime=lv.runtime(
            workspace=lv.workspace.local(root=".agents"),
            lm=lv.lm.anthropic(model="claude-opus-4-7"),
            agent=lv.agent.codex(model="gpt-5-codex"),
            trust_profile=lv.TrustProfile.MANAGED_SANDBOX,
            budget=lv.budget(usd=200, calls=2000),
        ),
    ).run()

    # `.run()` raises NotImplementedError in the scaffold; once wired,
    # `result.best.artifact` is the optimized `SkillBank`.
    print(len(result.best.artifact.files), "skill files in the best bank")


if __name__ == "__main__":
    try:
        asyncio.run(amain())
    except NotImplementedError as e:
        print(f"(expected) {e}")
