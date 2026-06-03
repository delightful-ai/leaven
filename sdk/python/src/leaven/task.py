"""Inspect-shaped task records for the stage-composition surface.

`Task` is the benchmark world: cases, assets, setup, sandbox requirements,
and split metadata. It is inert data. Stage layouts and runtimes decide how a
selected case is projected into a workspace.
"""

from typing import Any

from pydantic import BaseModel, ConfigDict, Field

from .case import Case
from .sandbox.config import SandboxConfig
from .setup import SetupScript


class Task(BaseModel):
    """A collection of cases plus default task-world requirements."""

    model_config = ConfigDict(frozen=True, arbitrary_types_allowed=True, extra="forbid")

    cases: list[Case]
    sandbox: SandboxConfig | None = None
    setup: SetupScript | None = None
    name: str | None = None
    metadata: dict[str, Any] = Field(default_factory=dict)


__all__ = ["Task"]
