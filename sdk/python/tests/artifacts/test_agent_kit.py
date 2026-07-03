"""Tests for the AgentKitArtifact wire projection (lower and read back)."""

import pytest

from leaven.artifacts.agent_kit import (
    AGENT_KIT_ARTIFACT_TYPE,
    AgentKitArtifact,
    AgentKitSkill,
)
from leaven._seam_optimize.artifact_projection import (
    AGENT_KIT_CANDIDATE_KEY,
    artifact_from_record,
    project_seed,
)


def test_agent_kit_lowers_to_wire_artifact_body() -> None:
    """Example: a kit projects to {system_prompt, skills:[{path, content}]}."""
    kit = AgentKitArtifact(
        system_prompt="Solve the task carefully.",
        skills=[AgentKitSkill(path="regex/log.md", content="# log parsing")],
    )
    assert kit.to_wire_artifact() == {
        "system_prompt": "Solve the task carefully.",
        "skills": [{"path": "regex/log.md", "content": "# log parsing"}],
    }


def test_project_seed_fixes_agentic_reflection_for_a_kit() -> None:
    """Law: a kit seed lowers to the agent_kit type with agentic reflection."""
    projection = project_seed(AgentKitArtifact(system_prompt="hi"))
    assert projection.artifact.artifact_type == AGENT_KIT_ARTIFACT_TYPE
    assert projection.reflection_kind == "agentic"
    assert projection.artifact.artifact == {"system_prompt": "hi", "skills": []}


def test_kit_round_trips_through_the_wire_artifact() -> None:
    """Example: a kit read back from a wire record equals its parts."""
    kit = AgentKitArtifact(
        system_prompt="evolved",
        skills=[AgentKitSkill(path="a.md", content="x"), AgentKitSkill(path="b.md", content="y")],
    )
    projected = artifact_from_record(
        AGENT_KIT_ARTIFACT_TYPE,
        kit.to_wire_artifact(),
        candidate_id="cand_kit_child",
    )
    assert isinstance(projected, AgentKitArtifact)
    assert projected.system_prompt == "evolved"
    assert [(s.path, s.content) for s in projected.skills] == [("a.md", "x"), ("b.md", "y")]
    assert projected.candidate_id == "cand_kit_child"


def test_kit_runner_candidate_key_matches_the_host_projection_key() -> None:
    """Law: the SDK kit candidate key matches the host's runner payload key.

    The host (`leaven-seam-service` agent_kit loop) projects each kit candidate
    revision into the runner payload under `candidate_agent_kit`; the SDK worker
    reads that exact key. A drift here silently breaks the kit rollout.
    """
    assert AGENT_KIT_CANDIDATE_KEY == "candidate_agent_kit"


@pytest.mark.parametrize(
    ("path", "reason"),
    [
        ("", "path is empty"),
        ("/tmp/owned.md", "path must be relative"),
        ("../owned.md", "parent traversal"),
        ("regex/../owned.md", "parent traversal"),
        ("regex//notes.md", "empty component"),
        ("regex/./notes.md", "current-directory component"),
        ("regex\\notes.md", "backslash"),
        ("regex/\0/notes.md", "NUL"),
    ],
)
def test_agent_kit_skill_rejects_paths_that_escape_the_skills_subtree(
    path: str, reason: str
) -> None:
    """Law: SDK AgentKit skill paths obey the host's relative POSIX path law."""
    with pytest.raises(ValueError, match=reason):
        AgentKitSkill(path=path, content="unsafe")


def test_from_wire_artifact_rejects_a_non_string_system_prompt() -> None:
    """Boundary: a malformed wire artifact is rejected, not coerced."""
    with pytest.raises(TypeError, match="string system_prompt"):
        AgentKitArtifact.from_wire_artifact({"system_prompt": 7, "skills": []})


def test_from_wire_artifact_rejects_a_skill_missing_content() -> None:
    """Boundary: a skill missing path/content is rejected."""
    with pytest.raises(TypeError, match="must carry path and content"):
        AgentKitArtifact.from_wire_artifact(
            {"system_prompt": "hi", "skills": [{"path": "a.md"}]}
        )


def test_from_wire_artifact_rejects_a_non_string_skill_content() -> None:
    """Boundary: a skill with non-string content is rejected, not coerced."""
    with pytest.raises(TypeError, match="string path and content"):
        AgentKitArtifact.from_wire_artifact(
            {"system_prompt": "hi", "skills": [{"path": "a.md", "content": 7}]}
        )


def test_from_wire_artifact_rejects_a_skill_path_with_parent_traversal() -> None:
    """Boundary: wire readback cannot smuggle traversal into local materialization."""
    with pytest.raises(ValueError, match="parent traversal"):
        AgentKitArtifact.from_wire_artifact(
            {"system_prompt": "hi", "skills": [{"path": "../owned.md", "content": "x"}]}
        )
