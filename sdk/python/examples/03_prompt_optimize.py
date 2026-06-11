"""Example 03 — the canonical minimal sketch (a real optimization).

The smallest meaningful Leaven program shape: compose a prompt optimization for
an arithmetic QA task with GEPA, vector scoring, and a local runtime, then run
the real GEPA loop over the durable `leaven seam serve --stdio` host.

The whole program is `seed x Environment(task, rollout, rubric) x optimizer x
runtime`. The rollout is target-FREE (`InputCaseView` has no `.target`); the
rubric reads the target (`ScoringCaseView`, gated + receipted by the host). With
the deterministic mock LM this run genuinely improves: the seed template never
surfaces the question, so every case scores 0; the scripted mock reflection
proposes a template that surfaces `{question}`, the child re-evaluates to a
strictly better score, and the printed best is a changed template that beats the
seed. No spend: the mock LM and the local runtime make the whole loop offline.
"""

import asyncio
from pathlib import Path

import leaven as lv

HERE = Path(__file__).parent
FIXTURE = HERE / "fixtures" / "arithmetic.jsonl"

# The mock reflection response: a fenced template that surfaces `{question}`, so
# the child the host authors can actually answer (the seed cannot). The host
# reflects with this deterministic response, so the loop is fully offline.
REFLECTED_TEMPLATE = (
    "Here is the improved instruction:\n"
    "```\n"
    "Solve this arithmetic problem: {question}. Output only the integer.\n"
    "```"
)


# ----- rollout: how the current artifact runs on one case -------------------
# `Rollout.fn` wraps a plain async runner. The rollout `cx` is TARGET-FREE: it
# sees `case.input`, can drive `cx.lm` / `cx.agent` / `cx.sandbox`, but `case`
# is an `InputCaseView` — it has no `.target` attribute at all (structural).
# This calculator answers from the rendered prompt: only a template that
# surfaces `{question}` lets it compute, so the seed scores 0 and the reflected
# child scores 1 — the headroom the optimizer closes.
@lv.runner
async def run(prompt: lv.PromptArtifact, case: lv.InputCaseView, cx: lv.RolloutContext) -> str:
    _ = cx
    question = case.input["question"]
    if not isinstance(question, str):
        return "0"
    rendered = prompt.template.format(**case.input)
    if question not in rendered:
        return "0"
    return _evaluate(question)


def _evaluate(question: str) -> str:
    # A tiny safe arithmetic evaluator over + - * / and parentheses; the fixture
    # answers are integers, so non-integer results fall back to "0".
    allowed = set("0123456789+-*/(). ")
    if not set(question) <= allowed:
        return "0"
    try:
        value = eval(question, {"__builtins__": {}}, {})
    except (ArithmeticError, SyntaxError, ValueError):
        return "0"
    if isinstance(value, int):
        return f"{value}"
    if isinstance(value, float) and value.is_integer():
        return f"{int(value)}"
    return "0"


# ----- rubric: how the rollout's output scores ------------------------------
# `@lv.reward` is the authoring sugar; `Rubric([...])` collects rewards. The
# rubric `cx` is scorer-role: `case` is a `ScoringCaseView`, so `case.target`
# is readable here (gated + receipted by the host).
@lv.reward
async def exact(output: str, case: lv.ScoringCaseView, cx: lv.RubricContext) -> float:
    _ = cx
    return 1.0 if output == (case.target or {})["answer"] else 0.0


@lv.reward(weight=0.1)
async def concise(output: str, case: lv.ScoringCaseView, cx: lv.RubricContext) -> lv.RewardValue:
    _ = (case, cx)
    return lv.RewardValue(value=1.0 if len(output) <= 8 else 0.0, feedback=f"{len(output)} chars")


# ----- composition ----------------------------------------------------------
def _split_cases() -> list[lv.Case]:
    # GEPA screens children on a train minibatch and scores accepted children on
    # the validation split, so the task needs both. A small 3-train / 1-val split
    # keeps the deterministic metric-call budget tiny.
    cases = lv.cases.from_jsonl(str(FIXTURE), limit=4).cases
    return [
        case.model_copy(update={"split": "validation" if index == 0 else "train"})
        for index, case in enumerate(cases)
    ]


async def amain() -> None:
    result = await lv.optimize(
        # The seed never surfaces `{question}`, so the rollout has nothing to
        # compute and every case scores 0 — real headroom to improve.
        seed=lv.PromptArtifact(template="You are a calculator. Always answer 0."),
        environment=lv.Environment(
            task=lv.Task(cases=_split_cases()),
            rollout=lv.Rollout.fn(run),
            rubric=lv.Rubric([exact, concise]),
        ),
        # population_size>=2 admits the seed plus one authored child; the budget
        # is the metric-call axis the GEPA loop spends (1 seed validation + 1
        # parent screen + 1 child screen + 1 child validation = 4).
        optimizer=lv.optimizers.gepa(population_size=2, minibatch_size=1),
        # The host reflects with this local mock LM. The runner is a calculator
        # (it never calls the LM), so the single mock response is reserved for
        # the reflection that authors the improved template.
        runtime=lv.runtime.local(
            budget=lv.budget(metric_calls=4),
            lm=lv.lm.mock(responses=[REFLECTED_TEMPLATE]),
        ),
    ).run()

    # `result.best.artifact` is a fully-typed `PromptArtifact`. The host ran the
    # real GEPA loop over the durable seam, persisted a Rust-owned checkpoint,
    # and returned the optimized projection: best != seed, best score > seed.
    seed = next(c for c in result.frontier if c.parent_id is None)
    seed_score = seed.summary_score or 0.0
    best_score = result.best.summary_score or 0.0
    # This is a real optimization, not a mechanics smoke: the child must beat the
    # seed. Assert it so the tour fails loudly if the loop ever regresses.
    assert result.best.id != seed.id, "best must be the authored child, not the seed"
    assert best_score > seed_score, "the optimization must improve on the seed"
    print(f"seed score:  {seed_score:.3f}")
    print(f"best score:  {best_score:.3f}")
    print(f"improved:    {result.best.id != seed.id}")
    print("seed prompt:")
    print(seed.artifact.template)
    print("optimized prompt:")
    print(result.best.artifact.template)


if __name__ == "__main__":
    asyncio.run(amain())
