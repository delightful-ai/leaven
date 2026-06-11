"""End-to-end proof that `lv.optimize(...).run()` drives the real GEPA loop.

This spawns the durable `leaven seam serve --stdio` host, which runs the real
`leaven-gepa` loop and dispatches runner and scorer stages back to the Python
worker over `leaven/stage.run`. The seed scores below a reflected child, so a
passing run proves the optimization genuinely improved — not a mechanics smoke
that returns the seed.
"""

from pathlib import Path

import pytest
from _pytest.monkeypatch import MonkeyPatch

import leaven as lv

# The mock reflection authors a template that surfaces `{question}` so the child
# can answer; the seed template never does, so the seed scores 0.
_REFLECTED = (
    "Improved instruction:\n```\nAnswer {question}. Output only the integer.\n```"
)


@lv.runner
async def run(prompt: lv.PromptArtifact, case: lv.InputCaseView, cx: lv.RolloutContext) -> str:
    _ = cx
    question = case.input["question"]
    if not isinstance(question, str):
        return "0"
    rendered = prompt.template.format(**case.input)
    if question not in rendered:
        return "0"
    return question if question.isdigit() else "0"


@lv.reward(id="exact")
async def exact(output: str, case: lv.ScoringCaseView, cx: lv.RubricContext) -> float:
    _ = cx
    assert case.target is not None
    return 1.0 if output == case.target["answer"] else 0.0


async def test_optimize_run_drives_a_real_improving_loop(
    tmp_path: Path,
    monkeypatch: MonkeyPatch,
) -> None:
    """Scenario: the real GEPA loop authors a changed child that beats the seed."""
    monkeypatch.setenv("LEAVEN_RUNS_ROOT", str(tmp_path / "runs"))
    result = await lv.optimize(
        seed=lv.PromptArtifact(template="Always answer 0."),
        environment=lv.Environment(
            task=lv.Task(
                name="e2e-improve",
                cases=[
                    lv.Case(id="t1", input={"question": "7"}, target={"answer": "7"}, split="train"),
                    lv.Case(
                        id="v1",
                        input={"question": "9"},
                        target={"answer": "9"},
                        split="validation",
                    ),
                ],
            ),
            rollout=lv.Rollout.fn(run),
            rubric=lv.Rubric([exact]),
        ),
        optimizer=lv.optimizers.gepa(population_size=2, minibatch_size=1),
        runtime=lv.runtime.local(
            budget=lv.budget(metric_calls=4),
            lm=lv.lm.mock(responses=[_REFLECTED]),
        ),
    ).run()

    seed = next(c for c in result.frontier if c.parent_id is None)
    best_score = result.best.summary_score
    seed_score = seed.summary_score
    assert best_score is not None
    assert seed_score is not None
    assert result.best.id != seed.id, "best must be the authored child, not the seed"
    assert best_score == 1.0
    assert seed_score == 0.0
    assert best_score > seed_score, "the child must beat the seed"
    assert "{question}" in result.best.artifact.template
    assert "{question}" not in seed.artifact.template
    # The durable run dir was written under the configured runs root.
    assert result.summary.run_dir is not None
    assert (Path(result.summary.run_dir) / "checkpoints" / "LATEST").is_file()


async def test_optimize_run_assessments_raise_until_readback_lands(
    tmp_path: Path,
    monkeypatch: MonkeyPatch,
) -> None:
    """Scenario: per-case assessments are not fabricated; the accessor raises."""
    monkeypatch.setenv("LEAVEN_RUNS_ROOT", str(tmp_path / "runs"))
    result = await lv.optimize(
        seed=lv.PromptArtifact(template="Always answer 0."),
        environment=lv.Environment(
            task=lv.Task(
                name="e2e-assessments",
                cases=[
                    lv.Case(id="t1", input={"question": "7"}, target={"answer": "7"}, split="train"),
                    lv.Case(
                        id="v1",
                        input={"question": "9"},
                        target={"answer": "9"},
                        split="validation",
                    ),
                ],
            ),
            rollout=lv.Rollout.fn(run),
            rubric=lv.Rubric([exact]),
        ),
        optimizer=lv.optimizers.gepa(population_size=2, minibatch_size=1),
        runtime=lv.runtime.local(
            budget=lv.budget(metric_calls=4),
            lm=lv.lm.mock(responses=[_REFLECTED]),
        ),
    ).run()

    facts = [fact.surface for fact in result.summary.unsupported]
    assert "run.inspection" in facts
    with pytest.raises(lv.AssessmentsUnavailableError, match="not available"):
        list(result.assessments())
