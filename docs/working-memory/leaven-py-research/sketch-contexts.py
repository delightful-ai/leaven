"""Sketch — the per-role `cx` context classes (redesign).

Redesign sketch (2026-06-01). The role-scoped stage contexts, renamed OFF the
engine-internal `RunContext` (the Rust `RunContext` is the graph-mutation
authority; the spec forbids it as a Python concept, leaven_python.md:573).

The safety boundary is STRUCTURAL — each role's `cx` only has the capabilities
and case projection its wire `Subject.role` allows. There is no "method that
raises if you're the wrong role"; the wrong capability simply isn't on the type.
Grounded in `2026-06-01-cx-surface-design.md`. NOT importable yet (scaffold).
"""

from __future__ import annotations

from pydantic import BaseModel, ConfigDict

from leaven._handles import WorkspaceHandle, WorkspaceLifetime, WorkspaceSurface
from leaven._receipts import CallReceipt
from leaven.builders.agent import AgentBuilder
from leaven.builders.assessments import AssessmentsBuilder
from leaven.builders.batch import BatchBuilder
from leaven.builders.lm import LmBuilder
from leaven.builders.proposals import ProposalsBuilder
from leaven.builders.sandbox import SandboxBuilder
from leaven.builders.workspace import WorkspaceReads  # read/query ops on a handle


# ----- case projections: the structural target gate -------------------------


class InputCaseView(BaseModel):
    """The case as a ROLLOUT (and reflector) sees it: input + metadata, NO
    target. There is no `.target` attribute at all — target-freedom is
    structural, not a runtime check. (wire: RunnerRequest.target_forbidden=true)
    """

    model_config = ConfigDict(frozen=True, extra="forbid")
    id: str
    input: dict
    metadata: dict


class ScoringCaseView(InputCaseView):
    """The case as a RUBRIC/scorer sees it: input + metadata + gated target.

    `.target` reads ergonomically (`case.target["answer"]`) but is backed by a
    gated, receipted dereference (wire: ScoreContext.target_handle), prefetched
    at case-load for this role only. Reading the target does NOT grant egressing
    it into an LM call — that's a separate monotonic data-class gate.
    """

    @property
    def target(self) -> dict:
        raise NotImplementedError("scaffold; gated + receipted target dereference")


# ----- shared capability mixins ---------------------------------------------


class _Effects:
    """Effects available to every stage role. None of these may egress the
    case target (data-class deny list enforced engine-side)."""

    lm: LmBuilder
    agent: AgentBuilder
    sandbox: SandboxBuilder
    workspace: WorkspaceReads  # read_file / git_diff / list / snapshot on a handle

    def batch(self) -> BatchBuilder:
        """Collapse multiple ops into one ACP round-trip."""
        raise NotImplementedError("scaffold")


class _StageMeta:
    @property
    def stage_id(self) -> str: ...
    @property
    def capability_fingerprint(self) -> str: ...


# ----- the role contexts (none named `RunContext`) --------------------------


class RolloutContext(_Effects, _StageMeta):
    """`Rollout.fn` body. TARGET-FREE. Runs the current artifact on one case."""

    case: InputCaseView

    @property
    def candidate_id(self) -> str: ...
    @property
    def rollout_workspace(self) -> WorkspaceHandle: ...


class RubricContext(_Effects, _StageMeta):
    """`@lv.reward` body. Scorer-role: target readable (gated), no graph
    mutation, no candidate materialization."""

    case: ScoringCaseView

    @property
    def rollout_workspace(self) -> WorkspaceHandle: ...


class ReflectContext(_Effects, _StageMeta):
    """Reflector. Target-SAFE reflective dataset only — it arrives via the
    request payload, NOT `cx.case`; no proposals, no assessments."""

    # reflective dataset = request.examples (target-safe projection)


class ProposeContext(_Effects, _StageMeta):
    """Proposer. May materialize the parent candidate and SUBMIT a typed
    proposal — never APPLY (the engine's RunContext::propose owns the
    finalizer: charge / record / emit / checkpoint)."""

    proposals: ProposalsBuilder  # submit-only; no submit_and_apply

    @property
    def parent_candidate_id(self) -> str | None: ...

    async def materialize_candidate(
        self,
        candidate_id: str,
        *,
        surface: WorkspaceSurface = "full_repo",
        lifetime: WorkspaceLifetime = "stage_call",
    ) -> WorkspaceHandle: ...


class JudgeContext(_Effects, _StageMeta):
    """Judge. Pairwise/listwise preference between candidates.

    OPEN: judge case projection — does it need the rubric/target to judge, or a
    target-safe view? old example 09 loaded `include=("input","target")`. Needs
    the seam check before this is locked (tracked as a cx open question).
    """


class EvaluatorContext(_Effects, _StageMeta):
    """INTERNAL / seam only — NOT a product-surface role. Full case incl.
    target; `assessments.submit` scoped to its own evaluation request."""

    case: ScoringCaseView
    assessments: AssessmentsBuilder  # own evaluation_request_id only

    @property
    def evaluation_request_id(self) -> str: ...

    async def materialize_candidate(
        self,
        candidate_id: str,
        *,
        surface: WorkspaceSurface = "full_repo",
        lifetime: WorkspaceLifetime = "stage_call",
    ) -> WorkspaceHandle: ...


__all__ = [
    "EvaluatorContext",
    "InputCaseView",
    "JudgeContext",
    "ProposeContext",
    "ReflectContext",
    "RolloutContext",
    "RubricContext",
    "ScoringCaseView",
]
