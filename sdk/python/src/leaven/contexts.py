"""Role-scoped stage contexts — the `cx` each stage receives.

Renamed off the engine-internal `RunContext` (the Rust `RunContext` is the
graph-mutation authority and the spec forbids it as a Python concept). Each role
gets ONLY the capabilities its wire `Subject.role` allows — the safety boundary
is STRUCTURAL (the capability simply isn't on the type), not a runtime raise.

Design: `docs/working-memory/leaven-py-research/2026-06-01-cx-surface-design.md`.
The case projection (target-free vs target-bearing) is the `case` PARAMETER type
on the stage function (`InputCaseView` / `ScoringCaseView`), not a `cx` field.
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
from .builders.workspace import WorkspaceBuilder, WorkspaceReads


class _StageMeta:
    @property
    def stage_id(self) -> str:
        """Engine-minted stage call id, immutable within the stage."""
        raise NotImplementedError("scaffold; see docs/specs/leaven_python.md")

    @property
    def capability_fingerprint(self) -> str:
        """Capability document fingerprint for this stage's authority."""
        raise NotImplementedError("scaffold; see docs/specs/leaven_python.md")


class _Effects(_StageMeta):
    """Effects available to every role. None of these may egress the case
    target — the engine enforces the data-class deny list at the call."""

    lm: LmBuilder
    agent: AgentBuilder
    sandbox: SandboxBuilder
    workspace: WorkspaceReads

    def batch(self) -> BatchBuilder:
        """Collapse multiple ops into one public-seam round-trip."""
        raise NotImplementedError("scaffold; see docs/specs/leaven_python.md")


class RolloutContext(_Effects):
    """`Rollout.fn` body. TARGET-FREE. Runs the current artifact on one case."""

    @property
    def candidate_id(self) -> str:
        """The candidate being run in this rollout."""
        raise NotImplementedError("scaffold; see docs/specs/leaven_python.md")

    @property
    def rollout_workspace(self) -> WorkspaceHandle:
        """Engine-prepared workspace for this rollout (read via `cx.workspace`)."""
        raise NotImplementedError("scaffold; see docs/specs/leaven_python.md")


class RubricContext(_Effects):
    """`@lv.reward` body. Scorer-role: inspect `rollout_workspace`, read the
    target via the `ScoringCaseView` case param (not `cx`), no graph mutation."""

    @property
    def rollout_workspace(self) -> WorkspaceHandle:
        """Engine-prepared workspace the rollout just used."""
        raise NotImplementedError("scaffold; see docs/specs/leaven_python.md")


class ReflectContext(_Effects):
    """Reflector. The target-SAFE reflective dataset arrives via the request
    payload (not `cx.case`); no proposals, no `rollout_workspace`."""


class ProposeContext(_Effects):
    """Proposer. May materialize the parent candidate, write, and SUBMIT a typed
    proposal — never APPLY (the engine's `RunContext::propose` owns the
    finalizer: charge / record / emit / checkpoint)."""

    workspace: WorkspaceBuilder
    proposals: ProposalsBuilder

    @property
    def parent_candidate_id(self) -> str | None:
        """Parent candidate id this proposal changes; None for fresh authoring."""
        raise NotImplementedError("scaffold; see docs/specs/leaven_python.md")

    @property
    def parent_workspace(self) -> WorkspaceHandle:
        """Engine-prepared workspace for the parent candidate."""
        raise NotImplementedError("scaffold; see docs/specs/leaven_python.md")


class JudgeContext(_Effects):
    """Judge. Pairwise/listwise preference between candidates.

    OPEN: judge case projection (target-safe vs rubric-bearing) is not locked —
    see the cx design doc. The loader is present; its projection is the gate.
    """

    case: CaseBuilder


class EvaluatorContext(_Effects):
    """INTERNAL / seam only — NOT a product-surface role. Full case loader,
    candidate materialization, and `assessments.submit` scoped to its own
    evaluation request."""

    workspace: WorkspaceBuilder
    case: CaseBuilder
    assessments: AssessmentsBuilder

    @property
    def evaluation_request_id(self) -> str:
        """The evaluation request id this evaluator was invoked for."""
        raise NotImplementedError("scaffold; see docs/specs/leaven_python.md")


__all__ = [
    "EvaluatorContext",
    "JudgeContext",
    "ProposeContext",
    "ReflectContext",
    "RolloutContext",
    "RubricContext",
]
