"""Setup script declarations for task samples.

These are inert declarations on `lv.Case`; the runtime decides whether and
where they may execute when a stage layout asks for a workspace.
"""

from typing import Literal

from pydantic import BaseModel, ConfigDict


class SetupScript(BaseModel):
    """A setup action required before a sample rollout."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    kind: Literal["bash"]
    script: str


def bash(script: str) -> SetupScript:
    """Declare a bash setup script for a sample or task."""
    return SetupScript(kind="bash", script=script)


__all__ = ["SetupScript", "bash"]
