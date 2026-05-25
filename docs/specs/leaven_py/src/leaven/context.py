"""Context objects — `RunContext`, `StageContext`, `EvalContext`.

The `cx` parameter passed into every decorated stage function. Carries the
builder namespaces (`cx.case`, `cx.workspace`, etc.) plus the batch context
manager and stage metadata.

Three flavors:
- `RunContext` — runner/scorer stages (per-candidate, per-case work)
- `StageContext` — reflector/proposer/judge stages (per stage call)
- `EvalContext` — evaluator stages (per evaluation request, may iterate many cases)

The scaffold keeps the builder namespaces visible on all three contexts for
typing convenience. The engine still enforces role-specific capabilities; not
every builder method is valid in every stage role.
"""

from __future__ import annotations

from ._handles import WorkspaceHandle
from .builders.agent import AgentBuilder
from .builders.assessments import AssessmentsBuilder
from .builders.batch import BatchBuilder
from .builders.case import CaseBuilder
from .builders.lm import LmBuilder
from .builders.proposals import ProposalsBuilder
from .builders.sandbox import SandboxBuilder
from .builders.workspace import WorkspaceBuilder


class _ContextBase:
    """Shared builder surface across all context flavors."""

    case: CaseBuilder
    workspace: WorkspaceBuilder
    lm: LmBuilder
    agent: AgentBuilder
    sandbox: SandboxBuilder
    assessments: AssessmentsBuilder
    proposals: ProposalsBuilder

    def batch(self) -> BatchBuilder:
        """Open a batch context. Multiple ops collapse into one ACP round-trip."""
        raise NotImplementedError("scaffold; see docs/specs/leaven_python.md")

    @property
    def stage_id(self) -> str:
        """The stage call id (engine-minted, immutable within the stage)."""
        raise NotImplementedError("scaffold; see docs/specs/leaven_python.md")

    @property
    def capability_fingerprint(self) -> str:
        """Capability document fingerprint for this stage's authority."""
        raise NotImplementedError("scaffold; see docs/specs/leaven_python.md")


class RunContext(_ContextBase):
    """Context passed to `@lv.runner` and `@lv.scorer` stages."""

    @property
    def candidate_id(self) -> str:
        """The candidate being evaluated in this run."""
        raise NotImplementedError("scaffold; see docs/specs/leaven_python.md")

    @property
    def case_id(self) -> str:
        """The case being evaluated in this run."""
        raise NotImplementedError("scaffold; see docs/specs/leaven_python.md")

    @property
    def rollout_workspace(self) -> WorkspaceHandle:
        """Workspace prepared for this rollout.

        The engine/runtime materializes the current artifact and case according
        to the active `Rollout.layout`. Runners and scorers may inspect this
        handle; they should not re-materialize ordinary rollout workspaces.
        """
        raise NotImplementedError("scaffold; see docs/specs/leaven_python.md")


class StageContext(_ContextBase):
    """Context passed to `@lv.reflector`, `@lv.proposer`, `@lv.judge` stages."""

    @property
    def parent_candidate_id(self) -> str | None:
        """Parent candidate id for proposers; None for reflectors operating
        on a candidate set without an explicit parent."""
        raise NotImplementedError("scaffold; see docs/specs/leaven_python.md")


class EvalContext(_ContextBase):
    """Context passed to `@lv.evaluator` stages.

    Evaluators are higher-privileged than runner/scorer — they iterate over
    cases, decide what to load, choose what to evaluate. The context carries
    the evaluation request handle plus all of the builder surface.
    """

    @property
    def evaluation_request_id(self) -> str:
        """The evaluation request id this evaluator was invoked for."""
        raise NotImplementedError("scaffold; see docs/specs/leaven_python.md")


__all__ = ["EvalContext", "RunContext", "StageContext"]
