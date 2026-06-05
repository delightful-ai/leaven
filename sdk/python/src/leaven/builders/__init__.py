"""Query/effect builders — `cx.case`, `cx.workspace`, `cx.lm`, `cx.agent`, etc.

Each builder is a namespace on the context object. Calls construct typed
Plan IR ops that the engine validates against the locked seam before
execution. Users see typed results; the wire stays invisible.
"""

from .agent import AgentBuilder
from .assessments import AssessmentsBuilder
from .case import CaseBuilder
from .lm import LmBuilder
from .proposals import ProposalsBuilder
from .sandbox import SandboxBuilder
from .workspace import WorkspaceBuilder

__all__ = [
    "AgentBuilder",
    "AssessmentsBuilder",
    "CaseBuilder",
    "LmBuilder",
    "ProposalsBuilder",
    "SandboxBuilder",
    "WorkspaceBuilder",
]
