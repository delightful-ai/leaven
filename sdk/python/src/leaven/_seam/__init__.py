"""Private client package for the Leaven public seam process.

Public dependency: the `leaven seam serve --stdio` CLI and the locked
public-seam JSON-RPC/Plan IR wire. Private dependency: sibling modules in this
package only.
"""

from __future__ import annotations

from .capability import effect_capability, proposer_stage_capability
from .client import SeamClient
from .config import (
    CodexCliRuntimeConfig,
    CommandRunnerStageConfig,
    LocalWorkspaceConfig,
    MockLmRuntimeConfig,
    MockRunnerStageConfig,
    OpenAiLmRuntimeConfig,
    SeamExecutionContext,
    SeamServiceConfig,
)
from .errors import SeamClientError
from .plans import (
    AgentRunRequest,
    LmCompleteRequest,
    ProposalSubmitRequest,
    StageRunProposeRequest,
    StageRunRequest,
)
from .resolve import resolve_codex_binary, resolve_leaven_binary, resolve_repo_root

__all__ = [
    "AgentRunRequest",
    "CodexCliRuntimeConfig",
    "CommandRunnerStageConfig",
    "LmCompleteRequest",
    "LocalWorkspaceConfig",
    "MockLmRuntimeConfig",
    "MockRunnerStageConfig",
    "OpenAiLmRuntimeConfig",
    "ProposalSubmitRequest",
    "SeamClient",
    "SeamClientError",
    "SeamExecutionContext",
    "SeamServiceConfig",
    "StageRunProposeRequest",
    "StageRunRequest",
    "effect_capability",
    "proposer_stage_capability",
    "resolve_codex_binary",
    "resolve_leaven_binary",
    "resolve_repo_root",
]
