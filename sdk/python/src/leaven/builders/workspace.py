"""`cx.workspace.*` — materialize workspaces, read files, run git queries.

Split into two surfaces for role-scoping: `WorkspaceReads` (inspect a prepared
handle) is available to every role; `WorkspaceBuilder` adds materialization and
writes and is granted only to privileged roles (proposer, evaluator).
"""

import asyncio
import base64
from collections.abc import Sequence
from typing import Literal, Protocol, Self

from msgspec import UNSET, UnsetType
from pydantic import BaseModel, ConfigDict, Field

from .._handles import WorkspaceHandle, WorkspaceLifetime, WorkspaceSurface
from .._receipts import CallReceipt, QueryReceipt
from .._seam import WorkspaceMaterializeRequest, WorkspaceQueryRequest, WorkspaceReleaseRequest
from .._seam._wire.expressions import (
    WorkspaceQueryCaptureArtifacts,
    WorkspaceQueryDigest,
    WorkspaceQueryGitDiff,
    WorkspaceQueryGitLog,
    WorkspaceQueryGitStatus,
    WorkspaceQueryList,
    WorkspaceQueryReadFile,
    WorkspaceQuerySnapshot,
    WorkspaceQueryStat,
)
from .._seam._wire.refs import BlobRef as WireBlobRef
from .._seam._wire.refs import ReceiptRefRecord
from .._seam._wire.results import (
    WorkspaceCaptureArtifactsResult,
    WorkspaceDiffPrimary,
    WorkspaceDigestResult,
    WorkspaceFilePrimary,
    WorkspaceGitDiffResult,
    WorkspaceGitLogResult,
    WorkspaceGitStatusResult,
    WorkspaceHandlePrimary,
    WorkspaceListingEntry,
    WorkspaceListingPrimary,
    WorkspaceListResult,
    WorkspaceMaterializeResult,
    WorkspaceReadFileResult,
    WorkspaceReleaseResult,
    WorkspaceSnapshotPrimary,
    WorkspaceSnapshotResult,
    WorkspaceStatResult,
)
from ..blob_ref import BlobRef


class WorkspaceFile(BaseModel):
    """Result of `cx.workspace.read_file(...)`."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    path: str
    content: str | bytes
    receipt: QueryReceipt


class WorkspaceEntry(BaseModel):
    """Typed workspace listing or captured artifact entry."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    path: str
    kind: str
    data_classes: list[str] = Field(default_factory=list)
    blob_ref: BlobRef | None = None


class WorkspaceDiff(BaseModel):
    """Result of `cx.workspace.git_diff(...)` and friends."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    text: str
    """Diff text in unified format."""
    receipt: QueryReceipt


class WorkspaceStatus(BaseModel):
    """Result of `cx.workspace.git_status(...)`."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    entries: list[WorkspaceEntry]
    """Per-file status entries (porcelain v2 shape)."""
    receipt: QueryReceipt


class WorkspaceListing(BaseModel):
    """Result of `cx.workspace.list(...)`."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    entries: list[WorkspaceEntry]
    receipt: QueryReceipt


class WorkspaceSnapshot(BaseModel):
    """Result of `cx.workspace.snapshot(...)` / `cx.workspace.digest(...)`."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    digest: str
    algorithm: str
    receipt: QueryReceipt


class WorkspaceReads:
    """Read/query operations on a workspace handle the engine already prepared.

    Available to EVERY stage role. None of these materialize or mutate; they
    inspect an existing handle (typically `cx.rollout_workspace`).
    """

    def __init__(
        self,
        client: "_WorkspaceSeamRequester | None" = None,
        *,
        idempotency_prefix: str = "workspace",
        plan_id: str = "plan_python_workspace",
    ) -> None:
        self._client = client
        self._idempotency_prefix = idempotency_prefix
        self._plan_id = plan_id

    @classmethod
    def _for_seam(
        cls,
        client: "_WorkspaceSeamRequester",
        *,
        idempotency_prefix: str = "workspace",
        plan_id: str = "plan_python_workspace",
    ) -> Self:
        return cls(client, idempotency_prefix=idempotency_prefix, plan_id=plan_id)

    async def read_file(
        self,
        handle: WorkspaceHandle,
        path: str,
        *,
        max_bytes: int | None = None,
        expected_data_classes: list[str] | None = None,
    ) -> WorkspaceFile:
        """Read one workspace-relative file."""
        result = await asyncio.to_thread(
            self._require_client().workspace_read_file,
            WorkspaceQueryRequest(
                request_id=f"{self._idempotency_prefix}-read-file",
                plan_id=self._plan_id,
                method_value="leaven/workspace.read_file",
                workspace=handle.workspace_id,
                op=WorkspaceQueryReadFile(
                    path=path,
                    expected_data_classes=expected_data_classes
                    if expected_data_classes is not None
                    else ["public"],
                    max_bytes=max_bytes if max_bytes is not None else UNSET,
                ),
                op_name="workspace_file",
            ),
        )
        primary = result.primary
        return WorkspaceFile(
            path=primary.path,
            content=_workspace_file_content(primary),
            receipt=QueryReceipt(receipt_id=_query_receipt(result)),
        )

    async def list(
        self,
        handle: WorkspaceHandle,
        path: str = ".",
        *,
        recursive: bool = False,
        max_entries: int | None = None,
    ) -> WorkspaceListing:
        """List directory contents under the workspace-relative path."""
        result = await asyncio.to_thread(
            self._require_client().workspace_list,
            WorkspaceQueryRequest(
                request_id=f"{self._idempotency_prefix}-list",
                plan_id=self._plan_id,
                method_value="leaven/workspace.list",
                workspace=handle.workspace_id,
                op=WorkspaceQueryList(
                    path=path,
                    recursive=recursive,
                    max_entries=max_entries if max_entries is not None else UNSET,
                ),
                op_name="workspace_listing",
            ),
        )
        return _workspace_listing(result.primary, _query_receipt(result))

    async def snapshot(
        self,
        handle: WorkspaceHandle,
        *,
        algorithm: str = "blake3",
    ) -> WorkspaceSnapshot:
        """Whole-workspace content digest."""
        if algorithm != "blake3":
            raise ValueError("workspace snapshot only supports blake3")
        result = await asyncio.to_thread(
            self._require_client().workspace_snapshot,
            WorkspaceQueryRequest(
                request_id=f"{self._idempotency_prefix}-snapshot",
                plan_id=self._plan_id,
                method_value="leaven/workspace.snapshot",
                workspace=handle.workspace_id,
                op=WorkspaceQuerySnapshot(),
                op_name="workspace_snapshot",
            ),
        )
        return _workspace_snapshot(result.primary, _query_receipt(result))

    async def stat(self, handle: WorkspaceHandle, path: str) -> WorkspaceListing:
        """Stat one workspace-relative path."""
        result = await asyncio.to_thread(
            self._require_client().workspace_stat,
            WorkspaceQueryRequest(
                request_id=f"{self._idempotency_prefix}-stat",
                plan_id=self._plan_id,
                method_value="leaven/workspace.stat",
                workspace=handle.workspace_id,
                op=WorkspaceQueryStat(path=path),
                op_name="workspace_stat",
            ),
        )
        return _workspace_listing(result.primary, _query_receipt(result))

    async def digest(
        self,
        handle: WorkspaceHandle,
        path: str = ".",
        *,
        algorithm: Literal["sha256", "blake3"] = "sha256",
    ) -> WorkspaceSnapshot:
        """Digest one workspace-relative path."""
        result = await asyncio.to_thread(
            self._require_client().workspace_digest,
            WorkspaceQueryRequest(
                request_id=f"{self._idempotency_prefix}-digest",
                plan_id=self._plan_id,
                method_value="leaven/workspace.digest",
                workspace=handle.workspace_id,
                op=WorkspaceQueryDigest(path=path, algorithm=algorithm),
                op_name="workspace_digest",
            ),
        )
        return _workspace_snapshot(result.primary, _query_receipt(result))

    async def git_diff(
        self,
        handle: WorkspaceHandle,
        *,
        against: Literal["seed", "parent", "baseline", "head"] = "parent",
    ) -> WorkspaceDiff:
        """Git diff against a ref."""
        result = await asyncio.to_thread(
            self._require_client().workspace_git_diff,
            WorkspaceQueryRequest(
                request_id=f"{self._idempotency_prefix}-git-diff",
                plan_id=self._plan_id,
                method_value="leaven/workspace.git_diff",
                workspace=handle.workspace_id,
                op=WorkspaceQueryGitDiff(against=against),
                op_name="workspace_git_diff",
            ),
        )
        return _workspace_diff(result.primary, _query_receipt(result))

    async def git_status(self, handle: WorkspaceHandle) -> WorkspaceStatus:
        """Git status (porcelain v2)."""
        result = await asyncio.to_thread(
            self._require_client().workspace_git_status,
            WorkspaceQueryRequest(
                request_id=f"{self._idempotency_prefix}-git-status",
                plan_id=self._plan_id,
                method_value="leaven/workspace.git_status",
                workspace=handle.workspace_id,
                op=WorkspaceQueryGitStatus(porcelain=True),
                op_name="workspace_git_status",
            ),
        )
        return WorkspaceStatus(
            entries=_workspace_entries(result.primary.entries),
            receipt=QueryReceipt(receipt_id=_query_receipt(result)),
        )

    async def git_log(self, handle: WorkspaceHandle, *, max_entries: int = 20) -> WorkspaceDiff:
        """Git log as a workspace_diff-family value (broad projection per seam)."""
        result = await asyncio.to_thread(
            self._require_client().workspace_git_log,
            WorkspaceQueryRequest(
                request_id=f"{self._idempotency_prefix}-git-log",
                plan_id=self._plan_id,
                method_value="leaven/workspace.git_log",
                workspace=handle.workspace_id,
                op=WorkspaceQueryGitLog(max_entries=max_entries),
                op_name="workspace_git_log",
            ),
        )
        return _workspace_diff(result.primary, _query_receipt(result))

    async def capture_artifacts(
        self,
        handle: WorkspaceHandle,
        paths: Sequence[str],
        *,
        max_bytes: int | None = None,
    ) -> WorkspaceListing:
        """Capture bounded artifact refs for workspace-relative paths."""
        result = await asyncio.to_thread(
            self._require_client().workspace_capture_artifacts,
            WorkspaceQueryRequest(
                request_id=f"{self._idempotency_prefix}-capture-artifacts",
                plan_id=self._plan_id,
                method_value="leaven/workspace.capture_artifacts",
                workspace=handle.workspace_id,
                op=WorkspaceQueryCaptureArtifacts(
                    paths=list(paths),
                    max_bytes=max_bytes if max_bytes is not None else UNSET,
                ),
                op_name="workspace_artifacts",
            ),
        )
        return _workspace_listing(result.primary, _query_receipt(result))

    def _require_client(self) -> "_WorkspaceSeamRequester":
        if self._client is None:
            raise NotImplementedError(
                "WorkspaceReads requires an engine-bound public-seam client"
            )
        return self._client


class WorkspaceBuilder(WorkspaceReads):
    """Full workspace surface: reads plus materialization and writes.

    Granted only to privileged roles (proposer, evaluator). Rollout/rubric
    contexts receive `WorkspaceReads`, so they structurally cannot materialize
    a candidate or write into a workspace.
    """

    async def materialize_candidate(
        self,
        candidate_id: str,
        *,
        surface: WorkspaceSurface = "full_repo",
        lifetime: WorkspaceLifetime = "stage_call",
    ) -> WorkspaceHandle:
        """Materialize a candidate's artifact into a fresh workspace.

        Returned handle auto-releases at the end of the stage_call (or run, or
        manually). Pass the handle into downstream `cx.sandbox.exec` /
        `cx.agent.run` / `cx.workspace.*` calls.

        Normal rollout pipelines should use `cx.rollout_workspace`; this method
        is for advanced proposer/evaluator code that deliberately materializes
        an additional workspace.
        """
        result = await asyncio.to_thread(
            self._require_client().workspace_materialize,
            WorkspaceMaterializeRequest(
                request_id=f"{self._idempotency_prefix}-materialize",
                plan_id=self._plan_id,
                idempotency_key=f"{self._idempotency_prefix}-materialize",
                candidate=candidate_id,
                surface=_wire_surface(surface),
                lifetime=_wire_lifetime(lifetime),
            ),
        )
        return _workspace_handle(
            result.primary,
            candidate_id=candidate_id,
            surface=surface,
            lifetime=lifetime,
        )

    async def release(self, handle: WorkspaceHandle) -> None:
        """Explicit release; only needed for `lifetime='manual'` handles."""
        await asyncio.to_thread(
            self._require_client().workspace_release,
            WorkspaceReleaseRequest(
                request_id=f"{self._idempotency_prefix}-release",
                plan_id=self._plan_id,
                idempotency_key=f"{self._idempotency_prefix}-release",
                workspace=handle.workspace_id,
            ),
        )

    async def write_file(
        self,
        handle: WorkspaceHandle,
        path: str,
        content: str | bytes,
    ) -> CallReceipt:
        """Write a workspace-relative file. Receipt binds the change."""
        raise NotImplementedError("scaffold; see docs/specs/leaven_python.md")

    async def write_skills(self, handle: WorkspaceHandle, bank: object) -> CallReceipt:
        """Skill-bank convenience: write a SkillBank into the workspace layout.

        Equivalent to walking the bank and calling `write_file` per file.
        Exists because the skill-bank → workspace mapping is fixed and common.
        """
        raise NotImplementedError("scaffold; see docs/specs/leaven_python.md")


class _WorkspaceSeamRequester(Protocol):
    def workspace_materialize(
        self, request: WorkspaceMaterializeRequest
    ) -> WorkspaceMaterializeResult: ...

    def workspace_release(self, request: WorkspaceReleaseRequest) -> WorkspaceReleaseResult: ...

    def workspace_read_file(self, request: WorkspaceQueryRequest) -> WorkspaceReadFileResult: ...

    def workspace_list(self, request: WorkspaceQueryRequest) -> WorkspaceListResult: ...

    def workspace_snapshot(self, request: WorkspaceQueryRequest) -> WorkspaceSnapshotResult: ...

    def workspace_stat(self, request: WorkspaceQueryRequest) -> WorkspaceStatResult: ...

    def workspace_digest(self, request: WorkspaceQueryRequest) -> WorkspaceDigestResult: ...

    def workspace_git_log(self, request: WorkspaceQueryRequest) -> WorkspaceGitLogResult: ...

    def workspace_git_diff(self, request: WorkspaceQueryRequest) -> WorkspaceGitDiffResult: ...

    def workspace_git_status(self, request: WorkspaceQueryRequest) -> WorkspaceGitStatusResult: ...

    def workspace_capture_artifacts(
        self, request: WorkspaceQueryRequest
    ) -> WorkspaceCaptureArtifactsResult: ...


def _workspace_handle(
    primary: WorkspaceHandlePrimary,
    *,
    candidate_id: str,
    surface: WorkspaceSurface,
    lifetime: WorkspaceLifetime,
) -> WorkspaceHandle:
    return WorkspaceHandle(
        workspace_id=primary.workspace,
        candidate_id=candidate_id,
        surface=surface,
        lifetime=lifetime,
        receipt=CallReceipt(receipt_id=primary.receipt),
    )


def _workspace_file_content(primary: WorkspaceFilePrimary) -> str | bytes:
    if primary.content is not UNSET:
        return primary.content
    if primary.content_base64 is not UNSET:
        return base64.b64decode(primary.content_base64)
    raise ValueError("workspace file result did not include content bytes")


def _workspace_listing(primary: WorkspaceListingPrimary, receipt: str) -> WorkspaceListing:
    return WorkspaceListing(
        entries=_workspace_entries(primary.entries),
        receipt=QueryReceipt(receipt_id=receipt),
    )


def _workspace_snapshot(primary: WorkspaceSnapshotPrimary, receipt: str) -> WorkspaceSnapshot:
    algorithm = primary.digest.split(":", 1)[0]
    return WorkspaceSnapshot(
        digest=primary.digest,
        algorithm=algorithm,
        receipt=QueryReceipt(receipt_id=receipt),
    )


def _workspace_diff(primary: WorkspaceDiffPrimary, receipt: str) -> WorkspaceDiff:
    if primary.text is UNSET:
        raise ValueError("workspace diff result did not include text")
    return WorkspaceDiff(
        text=primary.text,
        receipt=QueryReceipt(receipt_id=receipt),
    )


def _workspace_entries(value: list[WorkspaceListingEntry] | UnsetType) -> list[WorkspaceEntry]:
    if value is UNSET:
        return []
    return [
        WorkspaceEntry(
            path=entry.path,
            kind=entry.kind,
            data_classes=list(entry.data_classes),
            blob_ref=_blob_ref(entry.blob_ref),
        )
        for entry in value
    ]


def _blob_ref(value: WireBlobRef | UnsetType) -> BlobRef | None:
    if value is UNSET:
        return None
    return BlobRef(
        blob_id=value.id,
        sha256=value.sha256,
        bytes=value.bytes,
        data_classes=list(value.data_classes),
    )


def _query_receipt(
    result: (
        WorkspaceReadFileResult
        | WorkspaceListResult
        | WorkspaceSnapshotResult
        | WorkspaceStatResult
        | WorkspaceDigestResult
        | WorkspaceGitLogResult
        | WorkspaceGitDiffResult
        | WorkspaceGitStatusResult
        | WorkspaceCaptureArtifactsResult
    ),
) -> str:
    for receipt in result.receipts:
        if receipt.kind == "query":
            return _receipt_id(receipt.receipt)
    raise ValueError("workspace query result did not include a query receipt")


def _receipt_id(value: str | ReceiptRefRecord) -> str:
    if isinstance(value, str):
        return value
    return value.id


def _wire_surface(
    surface: WorkspaceSurface,
) -> Literal["program", "skills_only", "files_only", "custom"]:
    if surface == "full_repo":
        return "program"
    return surface


def _wire_lifetime(
    lifetime: WorkspaceLifetime,
) -> Literal["plan", "stage_call", "manual_release"]:
    if lifetime == "manual":
        return "manual_release"
    if lifetime == "run":
        return "plan"
    return lifetime


__all__ = [
    "WorkspaceBuilder",
    "WorkspaceDiff",
    "WorkspaceEntry",
    "WorkspaceFile",
    "WorkspaceListing",
    "WorkspaceReads",
    "WorkspaceSnapshot",
    "WorkspaceStatus",
]
