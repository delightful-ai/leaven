"""Deterministic unit tests for the Harbor trial evidence extraction (no Docker)."""

import json
from pathlib import Path

from codex_terminal_bench.trial import (
    TASK_GIT_COMMIT,
    TASK_GIT_URL,
    TrialPlan,
    build_trial_config,
)


def test_trial_config_pins_the_task_to_an_exact_commit() -> None:
    """Law: the rollout pins one Terminal-Bench-2 task at a fixed git commit."""
    config = build_trial_config(
        TrialPlan(
            agent_kit_dir=Path("/tmp/kit"),
            trials_dir=Path("/tmp/trials"),
            trial_name="t1",
            openai_api_key="sk-test",
        )
    )
    assert config.task.git_url == TASK_GIT_URL
    assert config.task.git_commit_id == TASK_GIT_COMMIT
    assert str(config.task.path) == "regex-log"
    assert config.agent.import_path == "codex_terminal_bench.agent:LeavenCodex"
    assert config.agent.model_name == "openai/gpt-5.4-mini"
    assert config.agent.kwargs == {"agent_kit_dir": "/tmp/kit"}
    assert config.agent.env == {"OPENAI_API_KEY": "sk-test"}


def test_ctrf_summary_parses_passed_total_from_the_report(tmp_path: Path) -> None:
    """Example: the CTRF report's passed/total drives partial credit."""
    from codex_terminal_bench.trial import _ctrf_summary

    ctrf = tmp_path / "ctrf.json"
    ctrf.write_text(
        json.dumps(
            {
                "results": {
                    "summary": {"tests": 4, "passed": 3, "failed": 1},
                    "tests": [
                        {"name": "test_a", "status": "passed"},
                        {"name": "test_b", "status": "passed"},
                        {"name": "test_c", "status": "passed"},
                        {"name": "test_d", "status": "failed"},
                    ],
                }
            }
        ),
        encoding="utf-8",
    )
    passed, total, detail = _ctrf_summary(ctrf)
    assert (passed, total) == (3, 4)
    assert "3/4" in detail
    assert "test_d" in detail


def test_ctrf_summary_is_empty_when_the_verifier_did_not_run(tmp_path: Path) -> None:
    """Boundary: a missing CTRF report yields no per-test credit, not a crash."""
    from codex_terminal_bench.trial import _ctrf_summary

    passed, total, detail = _ctrf_summary(tmp_path / "missing.json")
    assert (passed, total) == (0, 0)
    assert "did not run" in detail
