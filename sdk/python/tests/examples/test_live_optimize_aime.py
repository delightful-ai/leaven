"""Deterministic no-spend proof of example 14's optimization mechanics.

This drives the SAME runner, rubric, and composition as
`examples/14_live_optimize_aime.py` (`run`, `exact`, `build_optimization`) over
the real `leaven seam serve --stdio` host, but with a scripted mock LM over
AIME-shaped fixture cases. No network, no real AIME data, no spend.

The seed is an empty instruction, so the runner reports a non-answer without an
LM call and the seed scores 0. The single mock reflection response is a fenced
replacement instruction that embeds the fixture answer. After reflection authors
that child, the runner frames it around the problem and the mock solver replays
the reflection text, so the child solves and the re-evaluated child is admitted
onto the frontier and beats the seed. This proves the example's improvement
mechanic (a changed child applied and re-evaluated) and that worker LM cost plumbs
back into the result totals.

Note on the mock seam: the host rebuilds the runtime LM per `leaven/lm.complete`
call, so every solver call replays the first scripted response; the reflection LM
is built once and consumes the script independently. The mock therefore proves
loop mechanics, not prompt-sensitive solving — that is the live example's job.
"""

import importlib.util
from pathlib import Path
from typing import Protocol, cast

import pytest
from _pytest.monkeypatch import MonkeyPatch

import leaven as lv


class _Example14(Protocol):
    def build_optimization(
        self,
        *,
        cases: list[lv.Case],
        lm: lv.lm.LmConfig,
        metric_calls: int,
        usd: float,
        seed_template: str = ...,
        minibatch_size: int = ...,
        population_size: int = ...,
    ) -> lv.OptimizeBuilder[lv.PromptArtifact]: ...

    def normalize_answer(self, text: str) -> str | None: ...


def _example14() -> _Example14:
    path = Path(__file__).parents[2] / "examples" / "14_live_optimize_aime.py"
    spec = importlib.util.spec_from_file_location("leaven_example_14", path)
    if spec is None or spec.loader is None:
        raise RuntimeError("could not load examples/14_live_optimize_aime.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return cast("_Example14", module)


# The fixture answer the reflected instruction embeds. The mock solver replays
# the reflection text for every call (the host rebuilds the mock per call), and
# the runner normalizes that text to this integer, so the authored child solves.
_FIXTURE_ANSWER = 51

# One AIME-shaped train case (drives the screen) and one validation case (drives
# the admitted-child re-evaluation). These are fixtures, not real AIME data.
_FIXTURE_CASES = [
    lv.Case(
        id="fixture_train_0",
        input={"problem": "AIME-shaped fixture problem one; the answer is 51."},
        target={"answer": _FIXTURE_ANSWER},
        split="train",
    ),
    lv.Case(
        id="fixture_val_0",
        input={"problem": "AIME-shaped fixture problem two; the answer is 51."},
        target={"answer": _FIXTURE_ANSWER},
        split="validation",
    ),
]

# An empty seed instruction: the runner reports a non-answer without an LM call,
# so the seed scores 0, leaving real headroom for the optimizer.
_EMPTY_SEED = "   "

# The single mock reflection response: a fenced replacement instruction that ends
# with the fixture answer. The mock solver replays this text for every call, so
# the child's normalized answer is 51; the empty seed has no instruction to solve.
_REFLECTED_TEMPLATE = (
    "Here is the improved instruction:\n"
    "```\n"
    "Solve the competition math problem step by step.\n"
    f"ANSWER: {_FIXTURE_ANSWER}\n"
    "```"
)


async def test_example_14_optimization_mechanics_improve_with_mock_lm(
    tmp_path: Path,
    monkeypatch: MonkeyPatch,
) -> None:
    """Scenario: the example's loop authors a changed child that beats the seed."""
    monkeypatch.setenv("LEAVEN_RUNS_ROOT", str(tmp_path / "runs"))
    example = _example14()

    result = await example.build_optimization(
        cases=list(_FIXTURE_CASES),
        lm=lv.lm.mock(responses=[_REFLECTED_TEMPLATE]),
        metric_calls=4,
        usd=1.0,
        seed_template=_EMPTY_SEED,
        minibatch_size=1,
        population_size=2,
    ).run()

    seed = next(c for c in result.frontier if c.parent_id is None)
    seed_score = seed.summary_score
    best_score = result.best.summary_score
    assert seed_score is not None
    assert best_score is not None
    assert seed_score == 0.0
    assert best_score == 1.0
    assert result.best.id != seed.id, "best must be the authored child, not the seed"
    assert best_score > seed_score, "the re-evaluated child must beat the seed"
    # The reflected child instruction is a real, non-empty change.
    assert result.best.artifact.template != seed.artifact.template
    assert result.best.artifact.template.strip()
    assert not seed.artifact.template.strip()
    # A changed child was applied and re-evaluated within the metric-call budget.
    assert result.summary.total_calls <= 4
    # Worker LM cost plumbs back: the run reports a known, non-None cost total.
    assert result.summary.cost_status == "known"
    assert result.summary.total_cost_usd is not None
    # The durable run dir was written under the configured runs root.
    assert result.summary.run_dir is not None
    assert (Path(result.summary.run_dir) / "checkpoints" / "LATEST").is_file()


@pytest.mark.parametrize(
    ("text", "expected"),
    [
        ("ANSWER: 42", "42"),
        ("the answer is 042", "42"),
        ("ANSWER: 42.0", "42"),
        ("After working it out, ANSWER: 7", "7"),
        ("no integer here", None),
        ("ANSWER: 1000", None),  # out of the 0..999 AIME range
        ("ANSWER: 42.5", None),  # non-integer
    ],
)
def test_normalize_answer_matches_p8_style(text: str, expected: str | None) -> None:
    """Example: AIME answer normalization handles 42 / 042 / 42.0 and range."""
    assert _example14().normalize_answer(text) == expected
