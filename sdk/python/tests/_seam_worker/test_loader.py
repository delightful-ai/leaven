"""Tests for worker module reload of stages and rubrics."""

from pathlib import Path
from types import FunctionType

import pytest

import leaven as lv
from leaven._seam_worker.loader import load_rubric_from_file
from leaven._seam_worker.target import worker_argv_for_stage
from leaven.decorators import RegisteredStage


def test_load_rubric_preserves_same_function_name_rewards(tmp_path: Path) -> None:
    """Regression: worker reload keys rewards by id, not func.__name__.

    Distinct rewards can share a function body name (cross-module imports or
    factory wrappers). Name-keyed reload previously collapsed the vector to the
    last dimension and poisoned optimize scoring.
    """
    scenario = tmp_path / "scenario.py"
    scenario.write_text(
        "\n".join(
            [
                "import leaven as lv",
                "",
                "def make_reward(expected: str, *, reward_id: str, weight: float):",
                "    async def score(output, case, cx):",
                "        _ = (case, cx)",
                "        return 1.0 if output == expected else 0.0",
                "    return lv.reward(weight=weight, id=reward_id)(score)",
                "",
                "strict = make_reward(",
                "    'strict', reward_id='metrics.strict.exact_match', weight=1.0",
                ")",
                "lenient = make_reward(",
                "    'lenient', reward_id='metrics.lenient.exact_match', weight=0.5",
                ")",
                "rubric = lv.Rubric([strict, lenient])",
            ]
        ),
        encoding="utf-8",
    )

    reward_ids = [
        "metrics.strict.exact_match",
        "metrics.lenient.exact_match",
    ]
    reloaded = load_rubric_from_file(scenario, reward_ids=reward_ids)

    assert [reward.id for reward in reloaded.rewards] == reward_ids
    assert [reward.weight for reward in reloaded.rewards] == [1.0, 0.5]
    func_names: list[str] = []
    for reward in reloaded.rewards:
        assert isinstance(reward.func, FunctionType)
        func_names.append(reward.func.__name__)
    assert func_names == ["score", "score"]

    # Old name-keyed argv cannot recover either dimension once ids are required.
    with pytest.raises(LookupError, match="reward 'score' not found"):
        load_rubric_from_file(scenario, reward_ids=["score", "score"])


def test_load_rubric_refuses_duplicate_reward_ids(tmp_path: Path) -> None:
    """Distinct reward objects that share an id must not silently overwrite."""
    scenario = tmp_path / "dup_ids.py"
    scenario.write_text(
        "\n".join(
            [
                "import leaven as lv",
                "",
                "@lv.reward(id='shared')",
                "async def first(output, case, cx):",
                "    _ = (output, case, cx)",
                "    return 0.0",
                "",
                "@lv.reward(id='shared')",
                "async def second(output, case, cx):",
                "    _ = (output, case, cx)",
                "    return 1.0",
            ]
        ),
        encoding="utf-8",
    )

    with pytest.raises(ValueError, match="duplicate reward id 'shared'"):
        load_rubric_from_file(scenario, reward_ids=["shared"])


def test_worker_argv_passes_reward_ids() -> None:
    """Driver→worker argv must carry RegisteredReward.id values."""

    @lv.runner
    async def solve(
        prompt: lv.PromptArtifact,
        case: lv.InputCaseView,
        cx: lv.RolloutContext,
    ) -> str:
        _ = (prompt, case, cx)
        return "ok"

    assert isinstance(solve, RegisteredStage)
    argv = worker_argv_for_stage(
        solve,
        lm_model="mock",
        reward_ids=("metrics.strict.exact_match", "metrics.lenient.exact_match"),
    )
    assert argv.count("--rubric-reward") == 2
    assert "metrics.strict.exact_match" in argv
    assert "metrics.lenient.exact_match" in argv
