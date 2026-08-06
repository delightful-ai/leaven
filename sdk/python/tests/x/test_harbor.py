"""Deterministic tests for the optional Harbor adapter surface."""

import json
import sys
from pathlib import Path, PurePosixPath

import pytest

import leaven as lv
from leaven.x.harbor._kit_upload import upload_kit_tree
from leaven.x.harbor._types import HarborAdapterError


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


def test_agents_registry_describes_each_supported_agent() -> None:
    """Law: each registered agent declares its scope defaults and config surface."""
    codex = lv.x.harbor.agents.resolve("codex")
    assert codex.default_placement == "repo"
    assert codex.api_key_env == "OPENAI_API_KEY"
    assert codex.repo_prompt_file == "AGENTS.md"
    assert codex.repo_skills_subdir == ".agents/skills"
    assert codex.repo_skills_layout == "portable"

    claude = lv.x.harbor.agents.resolve("claude-code")
    assert claude.default_placement == "repo"
    assert claude.user_prompt_mode == "unsupported_append_flag"
    assert claude.api_key_env == "ANTHROPIC_API_KEY"
    assert claude.repo_prompt_file == "CLAUDE.md"
    assert claude.repo_skills_subdir == ".claude/skills"
    assert claude.repo_skills_layout == "claude_packages"


def test_unknown_agent_is_rejected_actionably() -> None:
    """Boundary: an unsupported agent names the supported set, never silently fails."""
    with pytest.raises(lv.x.harbor.HarborAdapterError, match="unknown Harbor agent"):
        lv.x.harbor.rollout.agent_kit(agent="nonesuch")


def _kit() -> lv.AgentKitArtifact:
    return lv.AgentKitArtifact(
        system_prompt="be careful",
        skills=[lv.AgentKitSkill(path="regex/notes.md", content="test edge cases")],
    )


def _case() -> lv.InputCaseView:
    return lv.InputCaseView(
        id="case/one",
        input={"harbor_task": {"path": "/harbor/task", "kind": "local"}},
    )


@pytest.mark.asyncio
async def test_agent_kit_claude_code_uses_repo_placement_by_default(tmp_path: Path) -> None:
    """Cutoff: Claude Code stages the kit in-repo because append flag quoting is unsafe."""
    calls: list[lv.x.harbor.rollout.HarborTrialPlan] = []

    async def fake_trial(
        plan: lv.x.harbor.rollout.HarborTrialPlan,
    ) -> lv.x.harbor.HarborTrialOutcome:
        calls.append(plan)
        assert plan.agent == "claude-code"
        assert plan.placement == "repo"
        assert plan.api_key_env == "ANTHROPIC_API_KEY"
        assert (plan.staging_dir / "AGENTS.md").read_text(encoding="utf-8") == "be careful"
        assert (plan.staging_dir / "skills" / "regex" / "notes.md").read_text(
            encoding="utf-8"
        ) == "test edge cases"
        return lv.x.harbor.HarborTrialOutcome(rewards={"reward": 1.0})

    rollout = lv.x.harbor.rollout.agent_kit(
        agent="claude-code",
        model="anthropic/claude-sonnet-4-6",
        trials_dir=tmp_path / "trials",
        trial_runner=fake_trial,
    )
    encoded = await rollout.stage.func(_kit(), _case(), None)  # type: ignore[union-attr,arg-type]

    assert calls, "fake trial seam must be used"
    assert lv.x.harbor.HarborTrialOutcome.decode(encoded).rewards["reward"] == 1.0


def test_agent_kit_claude_code_refuses_user_placement_until_harbor_quotes_it() -> None:
    """Boundary: the known-broken append-system-prompt path fails before Docker."""
    with pytest.raises(lv.x.harbor.HarborAdapterError, match="Use placement='repo'"):
        lv.x.harbor.rollout.agent_kit(agent="claude-code", placement="user")


@pytest.mark.asyncio
async def test_agent_kit_codex_repo_placement_uses_configurable_workdir(tmp_path: Path) -> None:
    """Cutoff: Codex defaults to repo placement with an explicit workdir, never /app."""
    calls: list[lv.x.harbor.rollout.HarborTrialPlan] = []

    async def fake_trial(
        plan: lv.x.harbor.rollout.HarborTrialPlan,
    ) -> lv.x.harbor.HarborTrialOutcome:
        calls.append(plan)
        return lv.x.harbor.HarborTrialOutcome(rewards={"reward": 0.0})

    rollout = lv.x.harbor.rollout.agent_kit(
        agent="codex",
        model="openai/gpt-5.4-mini",
        trials_dir=tmp_path / "trials",
        workdir="/workspace",
        trial_runner=fake_trial,
    )
    await rollout.stage.func(_kit(), _case(), None)  # type: ignore[union-attr,arg-type]

    plan = calls[0]
    assert plan.agent == "codex"
    assert plan.placement == "repo"
    assert plan.workdir == "/workspace"
    assert plan.task_path == "/harbor/task"


@pytest.mark.asyncio
async def test_agent_kit_passes_extra_agent_env_for_live_auth(tmp_path: Path) -> None:
    """Law: live auth env can ride through Harbor without becoming API-key glue."""
    calls: list[lv.x.harbor.rollout.HarborTrialPlan] = []
    oauth_env = {"CLAUDE_FORCE_OAUTH": "1", "CLAUDE_CODE_OAUTH_TOKEN": "token-test"}

    async def fake_trial(
        plan: lv.x.harbor.rollout.HarborTrialPlan,
    ) -> lv.x.harbor.HarborTrialOutcome:
        calls.append(plan)
        return lv.x.harbor.HarborTrialOutcome(rewards={"reward": 1.0})

    rollout = lv.x.harbor.rollout.agent_kit(
        agent="claude-code",
        trials_dir=tmp_path / "trials",
        agent_env=oauth_env,
        trial_runner=fake_trial,
    )
    await rollout.stage.func(_kit(), _case(), None)  # type: ignore[union-attr,arg-type]

    assert calls[0].api_key_env == "ANTHROPIC_API_KEY"
    assert calls[0].api_key == ""
    assert calls[0].agent_env == oauth_env


def test_import_leaven_does_not_import_harbor_dependency() -> None:
    """Boundary: core Leaven import exposes x.harbor without importing Harbor itself."""
    sys.modules.pop("harbor", None)

    assert lv.x.harbor.__name__ == "leaven.x.harbor"
    # Touching the registry and rollout builder must not import Harbor.
    lv.x.harbor.agents.resolve("codex")
    assert "harbor" not in sys.modules


class _DockerLikeEnvironment:
    """Mimic Harbor 0.13.1 Docker upload_file: parents must already exist."""

    def __init__(self) -> None:
        self.existing_dirs = {"/app"}
        self.ensured: list[list[str]] = []
        self.uploads: list[tuple[str, str]] = []
        self.uploaded_text: dict[str, str] = {}

    async def ensure_dirs(self, dirs: list[str], *, chmod: bool = True) -> None:
        del chmod
        self.ensured.append(list(dirs))
        self.existing_dirs.update(dirs)

    async def upload_file(self, source_path: Path | str, target_path: str) -> None:
        parent = str(PurePosixPath(target_path).parent)
        if parent not in self.existing_dirs:
            raise RuntimeError(f"docker compose cp failed: no such directory: {parent}")
        source = Path(source_path)
        self.uploads.append((str(source), target_path))
        self.uploaded_text[target_path] = source.read_text(encoding="utf-8")


@pytest.mark.asyncio
async def test_repo_kit_upload_creates_skill_parents_before_docker_cp(
    tmp_path: Path,
) -> None:
    """Regression: Harbor Docker upload_file is compose cp and needs nested parents."""
    kit_dir = tmp_path / "kit"
    (kit_dir / "skills" / "regex").mkdir(parents=True)
    (kit_dir / "AGENTS.md").write_text("use the regex skill", encoding="utf-8")
    (kit_dir / "skills" / "regex" / "SKILL.md").write_text(
        "match dates carefully", encoding="utf-8"
    )

    environment = _DockerLikeEnvironment()
    await upload_kit_tree(
        environment,
        kit_dir=kit_dir,
        workdir="/app",
        prompt_file="AGENTS.md",
        skills_subdir=".agents/skills",
    )

    assert environment.ensured
    assert "/app/.agents/skills/regex" in environment.ensured[0]
    assert environment.uploads == [
        (str(kit_dir / "AGENTS.md"), "/app/AGENTS.md"),
        (
            str(kit_dir / "skills" / "regex" / "SKILL.md"),
            "/app/.agents/skills/regex/SKILL.md",
        ),
    ]


@pytest.mark.asyncio
async def test_claude_repo_upload_projects_portable_skills_into_skill_packages(
    tmp_path: Path,
) -> None:
    """Regression: Claude Code only discovers `.claude/skills/<n>/SKILL.md` packages.

    Copying a portable AgentKit path such as `regex/notes.md` into
    `.claude/skills/regex/notes.md` is silently ignored, so GEPA evolves skill
    bodies the Claude Code agent never loads.
    """
    kit_dir = tmp_path / "kit"
    (kit_dir / "skills" / "regex").mkdir(parents=True)
    (kit_dir / "AGENTS.md").write_text("be careful", encoding="utf-8")
    (kit_dir / "skills" / "regex" / "notes.md").write_text("test edge cases", encoding="utf-8")

    environment = _DockerLikeEnvironment()
    await upload_kit_tree(
        environment,
        kit_dir=kit_dir,
        workdir="/app",
        prompt_file="CLAUDE.md",
        skills_subdir=".claude/skills",
        skills_layout="claude_packages",
    )

    skill_target = "/app/.claude/skills/regex-notes/SKILL.md"
    assert environment.uploads[0] == (str(kit_dir / "AGENTS.md"), "/app/CLAUDE.md")
    assert environment.uploads[1][1] == skill_target
    assert "/app/.claude/skills/regex-notes" in environment.ensured[0]
    uploaded = environment.uploaded_text[skill_target]
    assert uploaded.startswith("---\nname: regex-notes\n")
    assert "description: Leaven AgentKit skill (regex/notes.md)." in uploaded
    assert uploaded.endswith("test edge cases")


@pytest.mark.asyncio
async def test_claude_repo_upload_preserves_existing_skill_md_packages(
    tmp_path: Path,
) -> None:
    """Law: kits that already use `<n>/SKILL.md` keep their package layout."""
    kit_dir = tmp_path / "kit"
    (kit_dir / "skills" / "regex").mkdir(parents=True)
    (kit_dir / "AGENTS.md").write_text("be careful", encoding="utf-8")
    (kit_dir / "skills" / "regex" / "SKILL.md").write_text(
        "---\nname: regex\ndescription: parse logs\n---\n\nmatch dates",
        encoding="utf-8",
    )

    environment = _DockerLikeEnvironment()
    await upload_kit_tree(
        environment,
        kit_dir=kit_dir,
        workdir="/app",
        prompt_file="CLAUDE.md",
        skills_subdir=".claude/skills",
        skills_layout="claude_packages",
    )

    assert environment.uploads == [
        (str(kit_dir / "AGENTS.md"), "/app/CLAUDE.md"),
        (
            str(kit_dir / "skills" / "regex" / "SKILL.md"),
            "/app/.claude/skills/regex/SKILL.md",
        ),
    ]


@pytest.mark.asyncio
async def test_claude_repo_upload_refuses_projected_skill_name_collisions(
    tmp_path: Path,
) -> None:
    """Boundary: colliding Claude package names fail before a silent overwrite."""
    kit_dir = tmp_path / "kit"
    (kit_dir / "skills").mkdir(parents=True)
    (kit_dir / "AGENTS.md").write_text("be careful", encoding="utf-8")
    (kit_dir / "skills" / "regex-notes.md").write_text("a", encoding="utf-8")
    (kit_dir / "skills" / "regex").mkdir()
    (kit_dir / "skills" / "regex" / "notes.md").write_text("b", encoding="utf-8")

    with pytest.raises(HarborAdapterError, match="collides"):
        await upload_kit_tree(
            _DockerLikeEnvironment(),
            kit_dir=kit_dir,
            workdir="/app",
            prompt_file="CLAUDE.md",
            skills_subdir=".claude/skills",
            skills_layout="claude_packages",
        )
