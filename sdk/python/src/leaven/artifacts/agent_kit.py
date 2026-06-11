"""AgentKitArtifact — a Codex agent kit the optimizer evolves.

An agent kit is the agent's authored instruction surface: a system prompt
(materialized as `AGENTS.md`, which Codex reads natively from its working
directory) plus zero or more skill files (markdown the agent can consult). The
optimizer evolves the kit through agentic reflection: a real agent reads the
parent kit and the per-case feedback, then authors an improved kit.

This is the Python projection of the locked `agent_kit` wire artifact. On the
host side the real artifact is a Git-backed revision (`GitProgramArtifact`); the
`{system_prompt, skills:[{path, content}]}` wire record is a flat projection of
two of that artifact's three materialized slots (`system_prompt` and `skills`;
the `agent_docs` slot is omitted from V1). The host builds a run-scoped Git
repository from this projection, evolves the kit over Git revisions, and reads
evolved child revisions back into this flat shape. So a child kit read back
from a run carries only the projected `system_prompt` and `skills`.

The wire `artifact_type` is `agent_kit` and the artifact body is
`{system_prompt, skills}` with each skill a `{path, content}` record. Skill
paths are portable relative POSIX paths inside the skills subtree; absolute
paths and parent traversal are rejected by the host's `AgentKit` path law.
"""

from typing import Literal, Self

from pydantic import BaseModel, ConfigDict, Field

from ..json_value import JsonObject, JsonValue

AGENT_KIT_ARTIFACT_TYPE = "agent_kit"
"""Wire `artifact_type` for an agent-kit artifact projection."""

AGENT_KIT_ARTIFACT_SCHEMA = "fp_schema_sha256_agent_kit"
"""Wire `artifact_schema` fingerprint the host validates the projection against."""


class AgentKitSkill(BaseModel):
    """One skill file in an agent kit.

    `path` is a portable relative POSIX path inside the skills subtree (the
    subtree Codex mounts under `.agents/skills`); `content` is the markdown body.
    Absolute paths and parent traversal are rejected by the host's `AgentKit`
    path law when the projection rides to the host.
    """

    model_config = ConfigDict(frozen=True, extra="forbid")

    path: str
    """Skills-subtree-relative path (e.g. `regex/log-parsing.md`)."""
    content: str
    """Markdown content of the skill file."""


class AgentKitArtifact(BaseModel):
    """A Codex agent kit: a system prompt plus skill files the optimizer evolves.

    The `system_prompt` is materialized as the agent's `AGENTS.md` instruction
    surface; `skills` are mounted under the agent's skills subtree. The optimizer
    evolves both through agentic reflection.
    """

    model_config = ConfigDict(frozen=True, extra="forbid")

    system_prompt: str
    """The agent's system prompt, materialized as `AGENTS.md`."""

    skills: list[AgentKitSkill] = Field(default_factory=list)
    """Skill files the agent can consult, mounted under its skills subtree."""

    candidate_id: str | None = None
    """Set when the kit came from the engine; None for hand-built seeds."""

    @classmethod
    def empty(cls) -> Self:
        """An empty seed kit (empty system prompt, no skills)."""
        return cls(system_prompt="", skills=[])

    def to_wire_artifact(self) -> JsonObject:
        """Project this kit into the locked `agent_kit` wire artifact body.

        The wire body is `{system_prompt, skills: [{path, content}]}`. The
        host-owned `candidate_id` is not part of the artifact body, so it is not
        projected here.
        """
        skills: list[JsonObject] = [
            {"path": skill.path, "content": skill.content} for skill in self.skills
        ]
        return {"system_prompt": self.system_prompt, "skills": skills}

    @classmethod
    def from_wire_artifact(cls, artifact: JsonObject, *, candidate_id: str | None = None) -> Self:
        """Build a kit from a wire `agent_kit` artifact body (seed or candidate)."""
        if "system_prompt" not in artifact:
            raise TypeError("agent_kit artifact must carry a system_prompt")
        system_prompt = artifact["system_prompt"]
        if not isinstance(system_prompt, str):
            raise TypeError("agent_kit artifact must carry a string system_prompt")
        # The wire body always carries `skills` (empty array when the kit has no
        # skills); the host projection and `to_wire_artifact` both always emit it.
        if "skills" not in artifact:
            raise TypeError("agent_kit artifact must carry a skills array")
        raw_skills = artifact["skills"]
        if not isinstance(raw_skills, list):
            raise TypeError("agent_kit artifact skills must be an array")
        skills = [_skill_from_wire(value) for value in raw_skills]
        return cls(system_prompt=system_prompt, skills=skills, candidate_id=candidate_id)


def _skill_from_wire(value: JsonValue) -> AgentKitSkill:
    if not isinstance(value, dict):
        raise TypeError("agent_kit skill must be an object")
    if "path" not in value or "content" not in value:
        raise TypeError("agent_kit skill must carry path and content")
    path = value["path"]
    content = value["content"]
    if not isinstance(path, str) or not isinstance(content, str):
        raise TypeError("agent_kit skill must carry string path and content")
    return AgentKitSkill(path=path, content=content)


class AgentKitChange(BaseModel):
    """Marker for an agent-kit change effect.

    Unlike a `PromptArtifact`, an agent kit has no typed structural change
    record on the wire: the optimizer evolves a kit through agentic reflection,
    where a real agent rewrites the materialized kit files in place and the host
    reads the diff back as a Git-revision child. There is therefore no literal
    change payload to author from Python; this type exists only to name that the
    kit change path is agentic, not a structural edit.
    """

    model_config = ConfigDict(frozen=True, extra="forbid")

    kind: Literal["agentic"] = "agentic"


__all__ = [
    "AGENT_KIT_ARTIFACT_SCHEMA",
    "AGENT_KIT_ARTIFACT_TYPE",
    "AgentKitArtifact",
    "AgentKitChange",
    "AgentKitSkill",
]
