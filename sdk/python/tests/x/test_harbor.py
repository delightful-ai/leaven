"""Deterministic tests for the optional Harbor adapter surface."""

import json
import sys
from pathlib import Path

import pytest

import leaven as lv


def _write_harbor_task(root: Path) -> Path:
    task_dir = root / "regex-log"
    task_dir.mkdir()
    (task_dir / "task.toml").write_text(
        'name = "Regex Log"\ninstruction = "extract dates"\n',
        encoding="utf-8",
    )
    (task_dir / "solution").mkdir()
    (task_dir / "solution" / "answer.txt").write_text("SECRET-SOLUTION", encoding="utf-8")
    (task_dir / "verifier.py").write_text("PRIVATE VERIFIER", encoding="utf-8")
    return task_dir


def test_local_harbor_task_package_maps_to_target_free_leaven_task(tmp_path: Path) -> None:
    """Law: Harbor task truth stays behind Harbor, not in runner-visible case input."""
    task_dir = _write_harbor_task(tmp_path)

    task = lv.x.harbor.task(task_dir, split="train")

    assert task.name == "Regex Log"
    assert task.metadata["source"] == "harbor"
    assert task.metadata["task_path"] == str(task_dir)
    case = task.cases[0]
    assert case.id == lv.x.harbor.case_from_task_dir(task_dir, split="train").id
    assert case.target is None
    assert case.split == "train"
    assert case.input == {"harbor_task": {"path": str(task_dir), "kind": "local"}}
    serialized_input = json.dumps(case.input)
    assert "SECRET-SOLUTION" not in serialized_input
    assert "PRIVATE VERIFIER" not in serialized_input
    assert case.metadata["task_checksum"]


def test_harbor_outcome_round_trips_structured_evidence() -> None:
    """Example: rollout output keeps reward vectors, CTRF, verifier, cost, and refs."""
    outcome = lv.x.harbor.HarborTrialOutcome(
        trial_dir="/tmp/trials/t1",
        rewards={"reward": 0.5, "lint": 1.0},
        ctrf=lv.x.harbor.CtrfEvidence(
            passed=2,
            failed=1,
            total=3,
            failed_names=["test_dates"],
        ),
        verifier_output="verifier reward: 0.5",
        trajectory_path="/tmp/trials/t1/agent/trajectory.json",
        tokens=lv.x.harbor.TokenEvidence(input=12, output=7),
        cost_usd=0.001,
    )

    decoded = lv.x.harbor.HarborTrialOutcome.decode(outcome.encode())

    assert decoded.rewards == {"reward": 0.5, "lint": 1.0}
    assert decoded.ctrf is not None
    assert decoded.ctrf.passed == 2
    assert decoded.ctrf.failed_names == ["test_dates"]
    assert decoded.tokens is not None
    assert decoded.tokens.input == 12
    assert decoded.cost_usd == 0.001


def test_harbor_outcome_tolerates_missing_optional_evidence() -> None:
    """Boundary: absent CTRF/trajectory/tokens means no evidence, not a crash."""
    decoded = lv.x.harbor.HarborTrialOutcome.decode('{"rewards":{"reward":1.0}}')

    assert decoded.rewards == {"reward": 1.0}
    assert decoded.ctrf is None
    assert decoded.trajectory_path is None
    assert decoded.tokens is None


def test_harbor_outcome_rejects_malformed_json_actionably() -> None:
    """Boundary: malformed rollout output raises an adapter error."""
    with pytest.raises(lv.x.harbor.HarborAdapterError, match="Harbor trial outcome"):
        lv.x.harbor.HarborTrialOutcome.decode("{not json")


@pytest.mark.asyncio
async def test_helper_rewards_score_map_keys_and_ctrf_fraction() -> None:
    """Law: Harbor evidence is scored by ordinary Leaven reward helpers."""
    output = lv.x.harbor.HarborTrialOutcome(
        rewards={"reward": 0.75},
        ctrf=lv.x.harbor.CtrfEvidence(
            passed=3,
            failed=1,
            total=4,
            failed_names=["test_edge_case"],
        ),
    ).encode()
    case = lv.ScoringCaseView(id="c1", input={}, target=None)

    rubric = lv.Rubric(
        [
            lv.x.harbor.rewards.map_key("reward", weight=2.0),
            lv.x.harbor.rewards.ctrf_fraction(weight=0.25),
        ]
    )
    reward_value = await rubric.rewards[0].func(output, case, None)  # type: ignore[arg-type]
    ctrf_value = await rubric.rewards[1].func(output, case, None)  # type: ignore[arg-type]

    assert rubric.rewards[0].weight == 2.0
    assert reward_value == lv.RewardValue(value=0.75, feedback="Harbor reward `reward`: 0.75")
    assert ctrf_value.value == pytest.approx(0.75)
    assert "test_edge_case" in ctrf_value.feedback


@pytest.mark.asyncio
async def test_helper_rewards_report_missing_evidence() -> None:
    """Boundary: missing key/zero CTRF score zero with useful feedback."""
    output = lv.x.harbor.HarborTrialOutcome(
        rewards={},
        ctrf=lv.x.harbor.CtrfEvidence(passed=0, failed=0, total=0),
    ).encode()
    case = lv.ScoringCaseView(id="c1", input={}, target=None)

    missing = await lv.x.harbor.rewards.map_key("reward").func(output, case, None)  # type: ignore[arg-type]
    empty_ctrf = await lv.x.harbor.rewards.ctrf_fraction().func(output, case, None)  # type: ignore[arg-type]

    assert missing.value == 0.0
    assert "missing Harbor reward `reward`" in missing.feedback
    assert empty_ctrf.value == 0.0
    assert "no CTRF total" in empty_ctrf.feedback


def test_trajectory_excerpt_surfaces_only_agent_authored_steps(tmp_path: Path) -> None:
    """Law: optimizer-visible trajectory feedback excludes non-agent task material."""
    trajectory = tmp_path / "trajectory.json"
    trajectory.write_text(
        json.dumps(
            {
                "steps": [
                    {"source": "user", "message": "TASK SECRET"},
                    {"source": "verifier", "message": "HIDDEN TEST"},
                    {
                        "source": "agent",
                        "message": "I inspected the logs.",
                        "tool_calls": [{"function_name": "shell"}],
                    },
                ]
            }
        ),
        encoding="utf-8",
    )

    excerpt = lv.x.harbor.trajectory_excerpt(trajectory)

    assert "I inspected the logs" in excerpt
    assert "tool[shell]" in excerpt
    assert "TASK SECRET" not in excerpt
    assert "HIDDEN TEST" not in excerpt


def test_trajectory_excerpt_degrades_cleanly_for_missing_or_malformed(tmp_path: Path) -> None:
    """Boundary: missing/malformed trajectory is empty unless strict mode is requested."""
    malformed = tmp_path / "bad.json"
    malformed.write_text("{bad", encoding="utf-8")

    assert lv.x.harbor.trajectory_excerpt(tmp_path / "missing.json") == ""
    assert lv.x.harbor.trajectory_excerpt(malformed) == ""
    with pytest.raises(lv.x.harbor.HarborAdapterError):
        lv.x.harbor.trajectory_excerpt(malformed, strict=True)


@pytest.mark.asyncio
async def test_codex_agent_kit_rollout_uses_fake_trial_without_live_spend(tmp_path: Path) -> None:
    """Cutoff: rollout materializes an AgentKit and returns encoded Harbor evidence."""
    calls: list[lv.x.harbor.rollout.HarborTrialPlan] = []

    async def fake_trial(plan: lv.x.harbor.rollout.HarborTrialPlan) -> lv.x.harbor.HarborTrialOutcome:
        calls.append(plan)
        assert (plan.agent_kit_dir / "AGENTS.md").read_text(encoding="utf-8") == "be careful"
        assert (
            plan.agent_kit_dir / "skills" / "regex" / "notes.md"
        ).read_text(encoding="utf-8") == "test edge cases"
        assert plan.task_path == "/harbor/task"
        assert plan.workdir == "/workspace"
        return lv.x.harbor.HarborTrialOutcome(
            trial_dir=str(plan.trials_dir / plan.trial_name),
            rewards={"reward": 1.0},
            ctrf=lv.x.harbor.CtrfEvidence(passed=5, failed=0, total=5),
        )

    rollout = lv.x.harbor.rollout.codex_agent_kit(
        model="openai/fake",
        trials_dir=tmp_path / "trials",
        workdir="/workspace",
        trial_runner=fake_trial,
    )
    case = lv.InputCaseView(
        id="case/one",
        input={"harbor_task": {"path": "/harbor/task", "kind": "local"}},
    )
    kit = lv.AgentKitArtifact(
        system_prompt="be careful",
        skills=[lv.AgentKitSkill(path="regex/notes.md", content="test edge cases")],
    )

    encoded = await rollout.stage.func(kit, case, None)  # type: ignore[union-attr,arg-type]

    assert calls, "fake trial seam must be used"
    assert lv.x.harbor.HarborTrialOutcome.decode(encoded).rewards["reward"] == 1.0


def test_import_leaven_does_not_import_harbor_dependency() -> None:
    """Boundary: core Leaven import exposes x.harbor without importing Harbor itself."""
    sys.modules.pop("harbor", None)

    assert lv.x.harbor.__name__ == "leaven.x.harbor"
    assert "harbor" not in sys.modules
