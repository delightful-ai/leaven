"""`lv.artifacts.prompt(...)` — a prompt-template artifact.

Governing spec: `docs/specs/leaven_python.md` — Artifact adapters.
"""

from __future__ import annotations

from .base import Artifact

__all__ = ["PromptArtifact", "prompt"]


class PromptArtifact(Artifact):
    """A prompt template artifact; `.render(**input)` fills its slots."""

    kind: str = "prompt"
    template: str

    def render(self, **kwargs: object) -> str:
        """Render the template against case input."""
        raise NotImplementedError("see leaven_python.md — Artifact adapters / prompt")


def prompt(template: str, **kwargs: object) -> PromptArtifact:
    """Build a prompt-template artifact (`lv.artifacts.prompt("Answer: {q}")`)."""
    raise NotImplementedError("see leaven_python.md — Artifact adapters / prompt")
