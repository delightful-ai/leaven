"""codex_kit — the flagship off-the-shelf harness artifact.

`mutable=[...]` is REQUIRED and validated against the known surface. Paths
outside it require explicit `lv.unsafe("custom/path")` (warns at construction).

The Rust adapter `crates/leaven-artifact-codex-kit` is UPCOMING; the Python
adapter lowers to it via the locked ACP wire and does not embed a Python
re-implementation.

Governing spec: `docs/specs/leaven_python.md` — codex_kit: the flagship harness
artifact.
"""

from __future__ import annotations

from collections.abc import Sequence

from .base import Artifact
from .unsafe import UnsafePath

__all__ = [
    "DEFAULT_MUTABLE_SURFACE",
    "NON_ARTIFACT_SURFACE",
    "OPT_IN_MUTABLE_SURFACE",
    "CodexKitArtifact",
    "codex_kit",
]


DEFAULT_MUTABLE_SURFACE: frozenset[str] = frozenset(
    {
        "AGENTS.md",
        ".agents/skills/**/SKILL.md",
        "dev_instructions.md",
    }
)
"""Optimized unless excluded."""

OPT_IN_MUTABLE_SURFACE: frozenset[str] = frozenset(
    {
        "task_message.md",
        "hooks.toml",
        "mcp.json",
        "tool_policy.toml",
    }
)
"""Must be named in `mutable=` to be optimized."""

NON_ARTIFACT_SURFACE: frozenset[str] = frozenset(
    {
        "codex_kit.toml",
        ".codex/",
    }
)
"""Frames the artifact; never optimized."""


class CodexKitArtifact(Artifact):
    """A codex_kit harness artifact over a mutable Codex-shaped surface."""

    kind: str = "codex_kit"
    root: str
    mutable: Sequence[str]

    def summary(self) -> str:
        """Human-readable inspection summary of the mutable surface."""
        raise NotImplementedError("see leaven_python.md — codex_kit")


def codex_kit(root: str, *, mutable: Sequence[str | UnsafePath]) -> CodexKitArtifact:
    """Build a codex_kit artifact.

    `mutable=` is REQUIRED and validated against the known surface; entries must
    be in `DEFAULT_MUTABLE_SURFACE`/`OPT_IN_MUTABLE_SURFACE` or wrapped in
    `lv.artifacts.unsafe(...)`.
    """
    raise NotImplementedError("see leaven_python.md — codex_kit")
