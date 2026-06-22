"""Materialize a Leaven agent kit to an on-disk directory for Harbor upload.

The `LeavenCodex` agent uploads the kit from a directory: the system prompt as
`AGENTS.md` and each skill under `skills/<path>`. This module writes an
`AgentKitArtifact` into that layout so the rollout can hand the directory to the
agent through `AgentConfig.kwargs["agent_kit_dir"]`.
"""

from leaven.x.harbor import materialize_agent_kit as materialize_kit

__all__ = ["materialize_kit"]
