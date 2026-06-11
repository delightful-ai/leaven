"""Tests for the optimize.run request lowering and V1 refusals."""

import pytest
from msgspec import UNSET

import leaven as lv
from leaven._errors import UnsupportedConfigurationError
from leaven._seam.optimize_run import ReflectionLmConfig
from leaven._seam_optimize.driver import _optimizer_config, _reflection_model, _request_document
from leaven._seam_optimize.types import PlannedOptimizeCase
from leaven.artifacts.prompt import PromptArtifact
from leaven.proposal import ProposalBatch
from leaven.stage_payloads import ProposeRequest


@lv.proposer(stage_id="tests.driver.proposer")
async def _proposer(req: ProposeRequest, cx: lv.ProposeContext) -> ProposalBatch:
    _ = (req, cx)
    return ProposalBatch(effects=[])


def _runtime(*, metric_calls: int | None = 4, usd: float | None = None) -> lv.Runtime:
    return lv.runtime.local(budget=lv.budget(metric_calls=metric_calls, usd=usd))


def _cases() -> list[PlannedOptimizeCase]:
    return [
        PlannedOptimizeCase(
            case_id="case_t1",
            input={"question": "2 + 3"},
            target={"answer": "5"},
            metadata={},
            split="train",
        ),
        PlannedOptimizeCase(
            case_id="case_v1",
            input={"question": "7 * 8"},
            target={"answer": "56"},
            metadata={},
            split="validation",
        ),
    ]


def test_optimizer_config_lowers_honored_knobs() -> None:
    """Example: population_size, minibatch_size, objective, metric_calls lower."""
    config = _optimizer_config(lv.optimizers.gepa(population_size=3, minibatch_size=2), _runtime())
    assert config.max_metric_calls == 4
    assert config.objective == "instance"
    assert config.population_size == 3
    assert config.minibatch_size == 2
    assert config.max_cost_usd_micro is UNSET


def test_usd_budget_lowers_into_max_cost_usd_micro() -> None:
    """Example: a usd budget becomes the optimizer usd-micro ceiling."""
    config = _optimizer_config(lv.optimizers.gepa(), _runtime(usd=0.25))
    assert config.max_cost_usd_micro == 250_000


def test_missing_metric_calls_budget_is_refused_naming_metric_calls() -> None:
    """Law: a GEPA optimize run requires a metric-call budget."""
    with pytest.raises(UnsupportedConfigurationError, match="metric_calls"):
        _optimizer_config(lv.optimizers.gepa(), _runtime(metric_calls=None, usd=20))


@pytest.mark.parametrize(
    ("budget", "needle"),
    [
        (lv.budget(metric_calls=4, calls=10), "calls"),
        (lv.budget(metric_calls=4, lm_tokens=1000), "lm_tokens"),
        (lv.budget(metric_calls=4, wall_seconds=30), "wall_seconds"),
        (lv.budget(metric_calls=4, concurrent_calls=2), "concurrent_calls"),
    ],
)
def test_unsupported_budget_axes_are_refused_naming_metric_calls(
    budget: lv.Budget,
    needle: str,
) -> None:
    """Law: a budget axis with no optimize.run route is refused, not dropped."""
    runtime = lv.runtime.local(budget=budget)
    with pytest.raises(UnsupportedConfigurationError) as info:
        _optimizer_config(lv.optimizers.gepa(), runtime)
    message = str(info.value)
    assert needle in message
    # The refusal names the V1 optimize budget axis.
    assert "metric_calls" in message


@pytest.mark.parametrize(
    ("optimizer", "needle"),
    [
        (lv.optimizers.gepa(frontier=lv.frontier.top_k(3)), "frontier"),
        (lv.optimizers.gepa(parent_selector="best"), "parent_selector"),
        (lv.optimizers.gepa(max_iterations=5), "max_iterations"),
        (lv.optimizers.gepa(propose=lv.Propose.fn(_proposer)), "propose"),
    ],
)
def test_unsupported_gepa_knobs_are_refused_naming_support(
    optimizer: lv.optimizers.Gepa,
    needle: str,
) -> None:
    """Law: a knob with no optimize.run route is refused, not silently dropped."""
    with pytest.raises(UnsupportedConfigurationError) as info:
        _optimizer_config(optimizer, _runtime())
    message = str(info.value)
    assert needle in message
    # The refusal names what V1 actually supports.
    assert "population_size" in message
    assert "lm reflection" in message


def test_reflection_model_prefers_explicit_reflection_lm() -> None:
    """Example: gepa(reflection_lm=...) names the reflection model; runtime is fallback."""
    runtime = _runtime()
    assert _reflection_model(lv.optimizers.gepa(), runtime) == "mock"
    explicit = lv.optimizers.gepa(reflection_lm=lv.lm.openai(model="gpt-4.1-mini"))
    assert _reflection_model(explicit, runtime) == "gpt-4.1-mini"


def test_request_document_keeps_targets_only_in_the_case_manifest() -> None:
    """Law: the lowered request carries targets in cases and a prompt seed only."""
    document = _request_document(
        seed=PromptArtifact(template="answer {question}"),
        cases=_cases(),
        optimizer=lv.optimizers.gepa(population_size=2),
        runtime=_runtime(),
        run_id="lower_test",
    )
    assert document.run_id == "run_lower_test"
    assert document.seed.artifact_type == "prompt"
    assert document.seed.artifact == {"template": "answer {question}"}
    assert [case.case for case in document.cases] == ["case_t1", "case_v1"]
    assert document.cases[0].target == {"answer": "5"}
    assert document.cases[0].split == "train"
    assert isinstance(document.reflection, ReflectionLmConfig)
    assert document.reflection.model == "mock"
