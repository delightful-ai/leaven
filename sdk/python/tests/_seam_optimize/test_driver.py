"""Tests for the optimize.run request lowering and V1 refusals."""

from pathlib import Path

import pytest
from msgspec import UNSET

import leaven as lv
from leaven._errors import UnsupportedConfigurationError
from leaven._seam.optimize_run import ReflectionAgenticConfig, ReflectionLmConfig
from leaven._seam_optimize.artifact_projection import project_seed
from leaven._seam_optimize.driver import (
    _agent_config,
    _optimizer_config,
    _reflection_config,
    _reflection_model,
    _request_document,
)
from leaven._seam_optimize.types import PlannedOptimizeCase
from leaven.artifacts.agent_kit import AgentKitArtifact
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
        projection=project_seed(PromptArtifact(template="answer {question}")),
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


def test_agent_kit_seed_lowers_to_agentic_reflection() -> None:
    """Law: an agent_kit seed lowers to the agent_kit type and agentic reflection."""
    document = _request_document(
        projection=project_seed(AgentKitArtifact(system_prompt="Answer plainly.")),
        cases=_cases(),
        optimizer=lv.optimizers.gepa(
            population_size=2,
            reflection_agent=lv.agent.codex(model="gpt-5.4-mini", transport="cli"),
        ),
        runtime=_runtime(),
        run_id="kit_test",
    )
    assert document.seed.artifact_type == "agent_kit"
    assert document.seed.artifact == {"system_prompt": "Answer plainly.", "skills": []}
    assert isinstance(document.reflection, ReflectionAgenticConfig)


def test_agent_kit_reflection_requires_a_reflection_agent() -> None:
    """Law: an agent_kit seed without a reflection agent is refused, not LM-defaulted."""
    with pytest.raises(UnsupportedConfigurationError, match="reflection_agent"):
        _reflection_config("agentic", lv.optimizers.gepa(population_size=2), _runtime())


def test_prompt_seed_refuses_a_reflection_agent() -> None:
    """Law: a reflection agent on the prompt (lm) path is refused, not ignored."""
    optimizer = lv.optimizers.gepa(
        reflection_agent=lv.agent.codex(model="gpt-5.4-mini", transport="cli")
    )
    with pytest.raises(UnsupportedConfigurationError, match="reflection_agent"):
        _reflection_config("lm", optimizer, _runtime())


def test_agent_config_resolves_codex_cli_binary_from_env(tmp_path, monkeypatch) -> None:
    """Example: the kit path lowers the reflection agent into a Codex CLI config."""
    monkeypatch.setenv("CODEX_HOME", str(tmp_path))  # isolate from the dev ~/.codex
    monkeypatch.setenv("LEAVEN_RUNS_ROOT", str(tmp_path / "runs"))
    monkeypatch.setenv("TEST_CODEX_BIN", "/usr/local/bin/codex-fake")
    optimizer = lv.optimizers.gepa(
        reflection_agent=lv.agent.codex(
            model="gpt-5.4-mini",
            transport="cli",
            bin_path_env="TEST_CODEX_BIN",
        )
    )
    agent = _agent_config(optimizer, reflection_kind="agentic")
    assert agent is not None
    assert agent.codex_bin == "/usr/local/bin/codex-fake"
    assert agent.model == "gpt-5.4-mini"


def test_agent_config_isolates_codex_home_and_home_from_the_operator(tmp_path, monkeypatch) -> None:
    """Law: the kit reflection isolates both CODEX_HOME and HOME by default.

    Codex pulls context from two roots: `$CODEX_HOME` (`AGENTS.md` + `config.toml`,
    the operator's doctrine) and `$HOME` (the `~/.agents` skill registry +
    `~/.codex/superpowers`, the operator's personal skill arsenal). The driver
    prepares a fresh HOME with `CODEX_HOME=<home>/.codex` carrying only a copied
    `auth.json` -- no `AGENTS.md`, no `config.toml`, and crucially no `~/.agents`
    -- so the reflection sees only codex built-ins and the workspace (kit) skills.
    """
    source = tmp_path / "operator-codex"
    source.mkdir()
    (source / "auth.json").write_text('{"auth_mode":"chatgpt"}', encoding="utf-8")
    (source / "AGENTS.md").write_text("personal doctrine: hard cutover", encoding="utf-8")
    (source / "config.toml").write_text("model = 'custom'", encoding="utf-8")
    runs_root = tmp_path / "runs"
    monkeypatch.setenv("CODEX_HOME", str(source))
    monkeypatch.setenv("LEAVEN_RUNS_ROOT", str(runs_root))
    monkeypatch.setenv("TEST_CODEX_BIN", "/usr/local/bin/codex-fake")
    optimizer = lv.optimizers.gepa(
        reflection_agent=lv.agent.codex(
            model="gpt-5.4-mini", transport="cli", bin_path_env="TEST_CODEX_BIN"
        )
    )
    agent = _agent_config(optimizer, reflection_kind="agentic")
    assert agent is not None
    assert agent.codex_home is not None and agent.home_dir is not None
    codex_home = Path(agent.codex_home)
    home = Path(agent.home_dir)
    # CODEX_HOME is <HOME>/.codex; the HOME is durable under the runs root.
    assert codex_home.parent == home
    assert runs_root in home.parents
    # Subscription auth carried over, but no operator doctrine and no skill registry.
    assert (codex_home / "auth.json").read_text(encoding="utf-8") == '{"auth_mode":"chatgpt"}'
    assert not (codex_home / "AGENTS.md").exists()
    assert not (codex_home / "config.toml").exists()
    assert not (home / ".agents").exists()  # `~/.agents` skill registry severed


def test_agent_config_honors_explicit_codex_home(tmp_path, monkeypatch) -> None:
    """Law: an explicit `codex_home` opts out of isolation; HOME stays the operator's."""
    monkeypatch.setenv("CODEX_HOME", str(tmp_path))
    monkeypatch.setenv("TEST_CODEX_BIN", "/usr/local/bin/codex-fake")
    optimizer = lv.optimizers.gepa(
        reflection_agent=lv.agent.codex(
            model="gpt-5.4-mini",
            transport="cli",
            bin_path_env="TEST_CODEX_BIN",
            codex_home="/explicit/home",
        )
    )
    agent = _agent_config(optimizer, reflection_kind="agentic")
    assert agent is not None
    assert agent.codex_home == "/explicit/home"
    assert agent.home_dir is None


def test_agent_config_is_unset_for_the_prompt_path() -> None:
    """Law: the prompt path configures no host agent runtime."""
    assert _agent_config(lv.optimizers.gepa(), reflection_kind="lm") is None
