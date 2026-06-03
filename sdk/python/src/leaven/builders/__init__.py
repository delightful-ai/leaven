"""Query/effect builders — `cx.case`, `cx.workspace`, `cx.lm`, `cx.agent`, etc.

Each builder is a namespace on the context object. Calls construct typed
Plan IR ops that the engine validates against the locked seam before
execution. Users see typed results; the wire stays invisible.

The `batch()` builder is special: it accumulates multiple ops into one
public-seam call with one receipt root.
"""

from __future__ import annotations

from .agent import AgentBuilder
from .assessments import AssessmentsBuilder
from .batch import BatchBuilder, batch
from .case import CaseBuilder
from .lm import LmBuilder
from .proposals import ProposalsBuilder
from .sandbox import SandboxBuilder
from .workspace import WorkspaceBuilder

__all__ = [
    "AgentBuilder",
    "AssessmentsBuilder",
    "BatchBuilder",
    "CaseBuilder",
    "LmBuilder",
    "ProposalsBuilder",
    "SandboxBuilder",
    "WorkspaceBuilder",
    "batch",
]
