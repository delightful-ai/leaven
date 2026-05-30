"""Stage workspace layouts — `lv.layouts.case_workspace()` etc.

Layouts are passed to the declarative built-ins via `layout=`. They describe
how the engine arranges the stage workspace (case files, artifact projection,
edit target).

Governing spec: `docs/specs/leaven_python.md` — Rollout / Propose (`layout=`).
"""

from __future__ import annotations

from pydantic import BaseModel, ConfigDict

__all__ = ["Layout", "case_workspace", "edit_artifact", "workspace"]


class Layout(BaseModel):
    """An immutable workspace-layout marker; `kind` discriminates."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    kind: str


def case_workspace(**kwargs: object) -> Layout:
    """Layout: project the artifact alongside the case files for a rollout."""
    raise NotImplementedError("see leaven_python.md — Rollout / layouts")


def edit_artifact(**kwargs: object) -> Layout:
    """Layout: materialize the parent artifact under `target/current/` for edit."""
    raise NotImplementedError("see leaven_python.md — Propose / layouts")


def workspace(*args: object, **kwargs: object) -> Layout:
    """The parameterized workspace layout form (`lv.layouts.workspace(...)`)."""
    raise NotImplementedError("see leaven_python.md — layouts")
