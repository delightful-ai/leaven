"""Regression tests proving generic Harbor glue comes from `leaven.x.harbor`."""

from pathlib import Path

import leaven as lv

from codex_terminal_bench import agent, scenario, trial


def test_terminal_bench_reuses_generic_harbor_adapter_helpers() -> None:
    """Law: Terminal-Bench constants stay local, generic evidence helpers do not."""
    assert trial.HarborTrialOutcome is lv.x.harbor.HarborTrialOutcome
    assert scenario.decode_outcome.__func__ is lv.x.harbor.HarborTrialOutcome.decode.__func__
    assert scenario.trajectory_excerpt is lv.x.harbor.trajectory_excerpt
    assert scenario.verifier.id == "leaven.x.harbor.rewards.reward"
    assert scenario.ctrf.id == "leaven.x.harbor.rewards.ctrf_fraction"


def test_terminal_bench_agent_uses_generic_leaven_codex_agent() -> None:
    """Law: Codex kit upload is adapter machinery, not example-local glue."""
    codex = agent.LeavenCodex(
        logs_dir=Path("/tmp/logs"),
        agent_kit_dir=None,
        workdir="/app",
    )
    assert isinstance(codex, lv.x.harbor.LeavenCodex)


def test_terminal_bench_trial_name_preserves_long_candidate_identity() -> None:
    """Regression: readable truncation cannot collapse distinct candidates."""
    case_id = f"case-{'a' * 120}"
    names = {
        scenario._trial_name(case_id, f"child-{'x' * 120}-one"),
        scenario._trial_name(case_id, f"child-{'x' * 120}-two"),
    }

    assert len(names) == 2
    assert all(len(name) <= 96 for name in names)
