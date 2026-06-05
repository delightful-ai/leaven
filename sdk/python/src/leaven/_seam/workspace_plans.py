"""Workspace Plan IR request construction for private public-seam clients."""

from dataclasses import dataclass
from typing import Literal

from msgspec import UNSET

from leaven._seam._wire.calls import WorkspaceMaterializeCall, WorkspaceReleaseCall
from leaven._seam._wire.expressions import PlanExpressionWorkspaceQuery
from leaven._seam._wire.payloads import (
    CommitPolicyGraphWritesAtomic,
    CommitPolicyNoGraphWrites,
    PlanDocument,
    PlanOp,
)
from leaven._seam._wire.refs import WireJsonObject

from .plans import SeamRequestMethod, _plan_document

WorkspaceQueryMethod = Literal[
    "leaven/workspace.capture_artifacts",
    "leaven/workspace.digest",
    "leaven/workspace.git_diff",
    "leaven/workspace.git_log",
    "leaven/workspace.git_status",
    "leaven/workspace.list",
    "leaven/workspace.read_file",
    "leaven/workspace.snapshot",
    "leaven/workspace.stat",
]


@dataclass(frozen=True)
class WorkspaceMaterializeRequest:
    """A single public-seam `leaven/workspace.materialize` Plan request."""

    request_id: str
    plan_id: str
    idempotency_key: str
    candidate: str
    surface: Literal["program", "skills_only", "files_only", "custom"] = "program"
    mode: Literal["read_only", "copy_on_write", "mutable_eval"] = "copy_on_write"
    lifetime: Literal["plan", "stage_call", "manual_release"] = "stage_call"

    @property
    def method(self) -> SeamRequestMethod:
        """Locked workspace materialization method."""
        return "leaven/workspace.materialize"

    def to_params(self) -> PlanDocument:
        """Return the locked workspace materialization Plan params."""
        return _plan_document(
            plan_id=self.plan_id,
            ops=[self._workspace_call()],
            return_names=["workspace"],
            commit=CommitPolicyGraphWritesAtomic(on_stale="reject"),
        )

    def _workspace_call(self) -> PlanOp:
        return PlanOp(
            kind="call",
            name="workspace",
            idempotency_key=self.idempotency_key,
            call=WorkspaceMaterializeCall(
                candidate=self.candidate,
                surface=self.surface,
                mode=self.mode,
                lifetime=self.lifetime,
            ),
        )


@dataclass(frozen=True)
class WorkspaceReleaseRequest:
    """A single public-seam `leaven/workspace.release` Plan request."""

    request_id: str
    plan_id: str
    idempotency_key: str
    workspace: str
    force: bool | None = None

    @property
    def method(self) -> SeamRequestMethod:
        """Locked workspace release method."""
        return "leaven/workspace.release"

    def to_params(self) -> PlanDocument:
        """Return the locked workspace release Plan params."""
        return _plan_document(
            plan_id=self.plan_id,
            ops=[self._release_call()],
            return_names=["workspace"],
            commit=CommitPolicyGraphWritesAtomic(on_stale="reject"),
        )

    def _release_call(self) -> PlanOp:
        return PlanOp(
            kind="call",
            name="workspace",
            idempotency_key=self.idempotency_key,
            call=WorkspaceReleaseCall(
                workspace=self.workspace,
                force=self.force if self.force is not None else UNSET,
            ),
        )


@dataclass(frozen=True)
class WorkspaceQueryRequest:
    """A single public-seam workspace query Plan request."""

    request_id: str
    plan_id: str
    method_value: WorkspaceQueryMethod
    workspace: str
    op: WireJsonObject
    op_name: str = "workspace_query"

    @property
    def method(self) -> SeamRequestMethod:
        """Locked workspace query method."""
        return self.method_value

    def to_params(self) -> PlanDocument:
        """Return the locked workspace query Plan params."""
        return _plan_document(
            plan_id=self.plan_id,
            ops=[self._workspace_query()],
            return_names=[self.op_name],
            commit=CommitPolicyNoGraphWrites(),
        )

    def _workspace_query(self) -> PlanOp:
        return PlanOp(
            kind="let",
            name=self.op_name,
            expr=PlanExpressionWorkspaceQuery(
                workspace=self.workspace,
                op=self.op,
            ),
        )


__all__ = [
    "WorkspaceMaterializeRequest",
    "WorkspaceQueryMethod",
    "WorkspaceQueryRequest",
    "WorkspaceReleaseRequest",
]
