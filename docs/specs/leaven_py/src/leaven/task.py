"""Task — the immutable task world.

`lv.Task` owns the case inventory and any task-global runtime requirements
(sandbox). It is inert: it declares facts the optimizer and runtime read; it
does not allocate workspaces, own layout, or enforce splits.

`cases` may be supplied directly or via `lv.cases.from_jsonl(...)` (which
returns `Sequence[Case]`).

Governing spec: `docs/specs/leaven_python.md` — Task and Case.
"""

from __future__ import annotations

from collections.abc import Sequence

from pydantic import BaseModel, ConfigDict

from .case import Case
from .sandbox.config import SandboxConfig

__all__ = ["Task"]


class Task(BaseModel):
    """An immutable task world: case inventory plus task-global sandbox needs."""

    model_config = ConfigDict(frozen=True, extra="forbid", arbitrary_types_allowed=True)

    cases: Sequence[Case]
    sandbox: SandboxConfig | None = None
