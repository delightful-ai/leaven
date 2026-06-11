"""`lv.optimize(...).run()` derives a fresh run id per invocation.

The host writes a durable run dir at `<runs_root>/run_<run_id>` and resumes a
colliding dir's optimizer checkpoint. A fixed run id would make `.run()`
non-idempotent (silent resume of stale state). Each invocation must therefore
produce a unique run id so reruns are deterministic and safe to rerun.
"""

import re

import leaven as lv
from leaven.optimize import OptimizeBuilder

# The wire RunId body the host requires (`^run_<this>$`).
_RUN_ID_BODY = re.compile(r"^[A-Za-z0-9_.:-]+$")


def _builder(*, task_name: str | None) -> OptimizeBuilder[lv.PromptArtifact]:
    builder = OptimizeBuilder[lv.PromptArtifact]()
    builder.environment = lv.Environment(
        task=lv.Task(name=task_name, cases=[]),
        rollout=lv.Rollout.fn(_run),
        rubric=lv.Rubric([_reward]),
    )
    return builder


@lv.runner
async def _run(prompt: lv.PromptArtifact, case: lv.InputCaseView, cx: lv.RolloutContext) -> str:
    _ = (prompt, case, cx)
    return "0"


@lv.reward
async def _reward(output: str, case: lv.ScoringCaseView, cx: lv.RubricContext) -> float:
    _ = (output, case, cx)
    return 0.0


def test_run_id_is_unique_per_invocation_for_an_unnamed_task() -> None:
    """Law: an unnamed task does not collide on a fixed run dir across runs."""
    builder = _builder(task_name=None)
    ids = {builder._run_id() for _ in range(8)}
    assert len(ids) == 8
    for run_id in ids:
        assert run_id.startswith("leaven_optimize_")
        assert _RUN_ID_BODY.match(run_id), run_id


def test_run_id_keeps_the_task_name_prefix_but_stays_unique() -> None:
    """Law: a named task keeps a readable prefix and is still fresh per run."""
    builder = _builder(task_name="My Task")
    first = builder._run_id()
    second = builder._run_id()
    assert first != second
    assert first.startswith("My_Task_")
    assert _RUN_ID_BODY.match(first), first
