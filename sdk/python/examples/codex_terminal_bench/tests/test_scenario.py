"""Deterministic unit tests for the rollout/rubric pure logic (no Docker)."""

import json
from pathlib import Path
from typing import cast

import leaven as lv
import pytest
from leaven.x.harbor import CtrfEvidence, TokenEvidence

from codex_terminal_bench.scenario import _trajectory_excerpt, _verifier_feedback, verifier
from codex_terminal_bench.trial import TrialOutcome
from codex_terminal_bench.wire import RolloutOutcome


def test_trajectory_excerpt_surfaces_only_the_agents_own_steps(tmp_path: Path) -> None:
    """Law: the excerpt summarizes the agent's own actions, not task/test internals."""
    trajectory = tmp_path / "trajectory.json"
    trajectory.write_text(
        json.dumps(
            {
                "steps": [
                    {"source": "user", "message": "TASK INSTRUCTION (must not appear)"},
                    {"source": "system", "message": "VERIFIER INTERNALS (must not appear)"},
                    {
                        "source": "agent",
                        "message": "I will inspect the log file structure first.",
                        "tool_calls": [{"function_name": "shell"}],
                    },
                    {"source": "agent", "message": "Writing my regex to /app/regex.txt."},
                ]
            }
        ),
        encoding="utf-8",
    )
    excerpt = _trajectory_excerpt(str(trajectory))
    assert "inspect the log file structure" in excerpt
    assert "tool[shell]" in excerpt
    assert "TASK INSTRUCTION" not in excerpt
    assert "VERIFIER INTERNALS" not in excerpt


def test_trajectory_excerpt_is_empty_when_no_trajectory_exists() -> None:
    """Boundary: a missing trajectory path yields no excerpt, not a crash."""
    assert _trajectory_excerpt(None) == ""
    assert _trajectory_excerpt("/nonexistent/trajectory.json") == ""


def test_verifier_feedback_includes_output_and_a_general_improvement_ask() -> None:
    """Example: scorer feedback carries the verifier output and a method ask."""
    parsed = RolloutOutcome(
        reward=0.0,
        ctrf_passed=2,
        ctrf_total=5,
        verifier_output="verifier reward: 0\nCTRF 2/5 tests passed; failing: test_x",
        trajectory_path=None,
    )
    feedback = _verifier_feedback(parsed)
    assert "CTRF 2/5" in feedback
    assert "teach a general working method" in feedback
    # The feedback asks for a general method, not the task's specific answer.
    assert "Do not encode the task's specific answer" in feedback


@pytest.mark.asyncio
async def test_verifier_reward_feeds_verifier_and_trajectory_feedback(tmp_path: Path) -> None:
    """Regression: GEPA reflection sees verifier output and safe trajectory excerpts."""
    trajectory = tmp_path / "trajectory.json"
    trajectory.write_text(
        json.dumps(
            {
                "steps": [
                    {
                        "source": "agent",
                        "message": "I will inspect the log file before writing the regex.",
                    }
                ]
            }
        ),
        encoding="utf-8",
    )
    outcome = TrialOutcome(
        rewards={"reward": 0.0},
        ctrf=CtrfEvidence(passed=0, failed=5, total=5),
        verifier_output="verifier reward: 0\nCTRF 0/5 tests passed",
        trajectory_path=str(trajectory),
    )
    case = lv.ScoringCaseView(id="tb_regex_log", input={}, target=None)
    cx = cast(lv.RubricContext, object())

    reward = cast(lv.RewardValue, await verifier.func(outcome.encode(), case, cx))

    assert reward.value == 0.0
    assert "verifier reward: 0" in reward.feedback
    assert "inspect the log file" in reward.feedback
    assert "teach a general working method" in reward.feedback


def test_trial_outcome_ctrf_fraction_is_zero_without_tests() -> None:
    """Law: an empty CTRF report scores a zero fraction, not a division error."""
    outcome = TrialOutcome(
        rewards={"reward": 0.0},
        ctrf=CtrfEvidence(passed=0, failed=0, total=0),
        tokens=TokenEvidence(input=None, output=None),
        cost_usd=None,
        trajectory_path=None,
        verifier_output="",
    )
    assert outcome.ctrf_fraction == 0.0
