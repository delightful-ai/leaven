"""Per-case setup steps — `lv.setup.bash("chmod +x ...")`.

Setup steps run before a case's rollout to prepare the case workspace.

Governing spec: `docs/specs/leaven_python.md` — Task and Case.
"""

from __future__ import annotations

from pydantic import BaseModel, ConfigDict

__all__ = ["SetupStep", "bash"]


class SetupStep(BaseModel):
    """An immutable per-case setup step."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    kind: str
    script: str | None = None


def bash(script: str, **kwargs: object) -> SetupStep:
    """A bash setup step (`lv.setup.bash("chmod +x case/files/challenge")`)."""
    raise NotImplementedError("see leaven_python.md — Task and Case / setup")
