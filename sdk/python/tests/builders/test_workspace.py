import json

import msgspec

from leaven._handles import WorkspaceHandle
from leaven._receipts import CallReceipt
from leaven._seam import WorkspaceMaterializeRequest, WorkspaceQueryRequest, WorkspaceReleaseRequest
from leaven._seam._wire.refs import BlobRef as WireBlobRef
from leaven._seam._wire.results import (
    ResultReceipt,
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
from leaven.builders.workspace import WorkspaceBuilder
from leaven.json_value import JsonObject, JsonValue


async def test_workspace_builder_materializes_and_releases_through_seam() -> None:
    """Scenario: workspace lifecycle methods lower to locked Plan call ops."""

    client = FakeWorkspaceSeamClient()
    workspace = WorkspaceBuilder._for_seam(
        client,
        idempotency_prefix="workspace-builder-test",
        plan_id="planworkspacebuilder001",
    )

    handle = await workspace.materialize_candidate(
        "cand_workspace_parent",
        surface="skills_only",
        lifetime="manual",
    )
    await workspace.release(handle)

    assert handle.workspace_id == "ws_workspace_builder"
    assert handle.candidate_id == "cand_workspace_parent"
    assert handle.surface == "skills_only"
    assert handle.lifetime == "manual"
    assert handle.receipt.receipt_id == "wrec_workspace_materialize"

    materialize = _params_object(client.materialize_request.to_params())
    assert materialize["return"] == ["workspace"]
    materialize_op = _json_object(_json_array(materialize["ops"])[0])
    assert materialize_op["kind"] == "call"
    assert materialize_op["idempotency_key"] == "workspace-builder-test-materialize"
    assert materialize_op["call"] == {
        "kind": "workspace_materialize",
        "candidate": "cand_workspace_parent",
        "mode": "copy_on_write",
        "lifetime": "manual_release",
        "surface": "skills_only",
    }

    release = _params_object(client.release_request.to_params())
    assert release["return"] == ["workspace"]
    release_op = _json_object(_json_array(release["ops"])[0])
    assert release_op["kind"] == "call"
    assert release_op["idempotency_key"] == "workspace-builder-test-release"
    assert release_op["call"] == {
        "kind": "workspace_release",
        "workspace": "ws_workspace_builder",
    }


async def test_workspace_reads_lower_retained_query_methods_through_seam() -> None:
    """Scenario: workspace reads query the retained locked V1 method family."""

    client = FakeWorkspaceSeamClient()
    workspace = WorkspaceBuilder._for_seam(
        client,
        idempotency_prefix="workspace-query-test",
        plan_id="planworkspacequery001",
    )
    handle = WorkspaceHandle(
        workspace_id="ws_workspace_builder",
        candidate_id="cand_workspace_parent",
        receipt=CallReceipt(receipt_id="wrec_workspace_materialize"),
    )

    file_result = await workspace.read_file(handle, "README.md", max_bytes=4096)
    listing = await workspace.list(handle, "src", recursive=True, max_entries=10)
    stat = await workspace.stat(handle, "src/lib.rs")
    digest = await workspace.digest(handle, "src/lib.rs", algorithm="sha256")
    snapshot = await workspace.snapshot(handle)
    git_log = await workspace.git_log(handle, max_entries=5)
    git_diff = await workspace.git_diff(
        handle,
        against="seed",
        expected_data_classes=["workspace.file"],
    )
    git_status = await workspace.git_status(handle)
    captured = await workspace.capture_artifacts(handle, ["README.md"], max_bytes=2048)

    assert file_result.content == "seeded workspace readme\n"
    assert file_result.receipt.receipt_id == "qrec_workspace_query"
    assert listing.entries[0].path == "src/lib.rs"
    assert stat.entries[0].blob_ref is not None
    assert stat.entries[0].blob_ref.blob_id == "blob_workspace_entry"
    assert digest.digest == "sha256:workspace"
    assert digest.algorithm == "sha256"
    assert snapshot.digest == "blake3:workspace"
    assert snapshot.algorithm == "blake3"
    assert git_log.text == "commit abc123\n"
    assert git_diff.text == "diff --git a/src/lib.rs b/src/lib.rs\n"
    assert git_status.entries[0].path == "src/lib.rs"
    assert captured.entries[0].blob_ref is not None
    assert captured.entries[0].blob_ref.sha256 == "a" * 64

    methods = [request.method for request in client.query_requests]
    assert methods == [
        "leaven/workspace.read_file",
        "leaven/workspace.list",
        "leaven/workspace.stat",
        "leaven/workspace.digest",
        "leaven/workspace.snapshot",
        "leaven/workspace.git_log",
        "leaven/workspace.git_diff",
        "leaven/workspace.git_status",
        "leaven/workspace.capture_artifacts",
    ]
    assert [_query_op(request) for request in client.query_requests] == [
        {"kind": "read_file", "path": "README.md", "max_bytes": 4096},
        {"kind": "list", "path": "src", "recursive": True, "max_entries": 10},
        {"kind": "stat", "path": "src/lib.rs"},
        {"kind": "digest", "path": "src/lib.rs", "algorithm": "sha256"},
        {"kind": "snapshot"},
        {"kind": "git_log", "max_entries": 5},
        {"kind": "git_diff", "against": "seed", "expected_data_classes": ["workspace.file"]},
        {"kind": "git_status", "porcelain": True},
        {"kind": "capture_artifacts", "paths": ["README.md"], "max_bytes": 2048},
    ]


class FakeWorkspaceSeamClient:
    def __init__(self) -> None:
        self.materialize_request = WorkspaceMaterializeRequest(
            request_id="unset",
            plan_id="unset",
            idempotency_key="unset",
            candidate="unset",
        )
        self.release_request = WorkspaceReleaseRequest(
            request_id="unset",
            plan_id="unset",
            idempotency_key="unset",
            workspace="unset",
        )
        self.query_requests: list[WorkspaceQueryRequest] = []

    def workspace_materialize(
        self,
        request: WorkspaceMaterializeRequest,
    ) -> WorkspaceMaterializeResult:
        self.materialize_request = request
        return WorkspaceMaterializeResult(
            method="leaven/workspace.materialize",
            primary=WorkspaceHandlePrimary(
                kind="workspace_handle",
                workspace="ws_workspace_builder",
                lifetime="manual_release",
                released=False,
                graph_revision="rev_workspace_builder",
                data_classes=["workspace.file"],
                replayability="fully_managed",
                receipt="wrec_workspace_materialize",
            ),
            receipts=[],
            redactions=[],
            capability_fingerprint="fp_cap_test",
            policy_fingerprint="fp_policy_test",
            data_classes=["workspace.file"],
        )

    def workspace_release(self, request: WorkspaceReleaseRequest) -> WorkspaceReleaseResult:
        self.release_request = request
        return WorkspaceReleaseResult(
            method="leaven/workspace.release",
            primary=WorkspaceHandlePrimary(
                kind="workspace_handle",
                workspace=request.workspace,
                lifetime="manual_release",
                released=True,
                graph_revision="rev_workspace_builder",
                data_classes=["workspace.file"],
                replayability="fully_managed",
                receipt="wrec_workspace_release",
            ),
            receipts=[],
            redactions=[],
            capability_fingerprint="fp_cap_test",
            policy_fingerprint="fp_policy_test",
            data_classes=["workspace.file"],
        )

    def workspace_read_file(self, request: WorkspaceQueryRequest) -> WorkspaceReadFileResult:
        self.query_requests.append(request)
        return WorkspaceReadFileResult(
            method="leaven/workspace.read_file",
            primary=WorkspaceFilePrimary(
                kind="workspace_file",
                path="README.md",
                graph_revision="rev_workspace_builder",
                data_classes=["workspace.file"],
                replayability="fully_managed",
                receipt="qrec_workspace_query",
                content="seeded workspace readme\n",
            ),
            receipts=[_query_receipt()],
            redactions=[],
            capability_fingerprint="fp_cap_test",
            policy_fingerprint="fp_policy_test",
            data_classes=["workspace.file"],
        )

    def workspace_list(self, request: WorkspaceQueryRequest) -> WorkspaceListResult:
        self.query_requests.append(request)
        return WorkspaceListResult(
            method="leaven/workspace.list",
            primary=_listing_primary(),
            receipts=[_query_receipt()],
            redactions=[],
            capability_fingerprint="fp_cap_test",
            policy_fingerprint="fp_policy_test",
            data_classes=["workspace.file"],
        )

    def workspace_stat(self, request: WorkspaceQueryRequest) -> WorkspaceStatResult:
        self.query_requests.append(request)
        return WorkspaceStatResult(
            method="leaven/workspace.stat",
            primary=_listing_primary(),
            receipts=[_query_receipt()],
            redactions=[],
            capability_fingerprint="fp_cap_test",
            policy_fingerprint="fp_policy_test",
            data_classes=["workspace.file"],
        )

    def workspace_digest(self, request: WorkspaceQueryRequest) -> WorkspaceDigestResult:
        self.query_requests.append(request)
        return WorkspaceDigestResult(
            method="leaven/workspace.digest",
            primary=_snapshot_primary("sha256:workspace"),
            receipts=[_query_receipt()],
            redactions=[],
            capability_fingerprint="fp_cap_test",
            policy_fingerprint="fp_policy_test",
            data_classes=["workspace.file"],
        )

    def workspace_snapshot(self, request: WorkspaceQueryRequest) -> WorkspaceSnapshotResult:
        self.query_requests.append(request)
        return WorkspaceSnapshotResult(
            method="leaven/workspace.snapshot",
            primary=_snapshot_primary("blake3:workspace"),
            receipts=[_query_receipt()],
            redactions=[],
            capability_fingerprint="fp_cap_test",
            policy_fingerprint="fp_policy_test",
            data_classes=["workspace.file"],
        )

    def workspace_git_log(self, request: WorkspaceQueryRequest) -> WorkspaceGitLogResult:
        self.query_requests.append(request)
        return WorkspaceGitLogResult(
            method="leaven/workspace.git_log",
            primary=_diff_primary("commit abc123\n"),
            receipts=[_query_receipt()],
            redactions=[],
            capability_fingerprint="fp_cap_test",
            policy_fingerprint="fp_policy_test",
            data_classes=["workspace.file"],
        )

    def workspace_git_diff(self, request: WorkspaceQueryRequest) -> WorkspaceGitDiffResult:
        self.query_requests.append(request)
        return WorkspaceGitDiffResult(
            method="leaven/workspace.git_diff",
            primary=_diff_primary("diff --git a/src/lib.rs b/src/lib.rs\n"),
            receipts=[_query_receipt()],
            redactions=[],
            capability_fingerprint="fp_cap_test",
            policy_fingerprint="fp_policy_test",
            data_classes=["workspace.file"],
        )

    def workspace_git_status(self, request: WorkspaceQueryRequest) -> WorkspaceGitStatusResult:
        self.query_requests.append(request)
        return WorkspaceGitStatusResult(
            method="leaven/workspace.git_status",
            primary=WorkspaceDiffPrimary(
                kind="workspace_diff",
                graph_revision="rev_workspace_builder",
                data_classes=["workspace.file"],
                replayability="fully_managed",
                text=" M src/lib.rs\n",
                entries=_entries(),
            ),
            receipts=[_query_receipt()],
            redactions=[],
            capability_fingerprint="fp_cap_test",
            policy_fingerprint="fp_policy_test",
            data_classes=["workspace.file"],
        )

    def workspace_capture_artifacts(
        self,
        request: WorkspaceQueryRequest,
    ) -> WorkspaceCaptureArtifactsResult:
        self.query_requests.append(request)
        return WorkspaceCaptureArtifactsResult(
            method="leaven/workspace.capture_artifacts",
            primary=_listing_primary(),
            receipts=[_query_receipt()],
            redactions=[],
            capability_fingerprint="fp_cap_test",
            policy_fingerprint="fp_policy_test",
            data_classes=["workspace.file"],
        )


def _query_receipt() -> ResultReceipt:
    return ResultReceipt(
        kind="query",
        receipt="qrec_workspace_query",
        status="succeeded",
        result_hash="fp_result_sha256_workspace",
    )


def _listing_primary() -> WorkspaceListingPrimary:
    return WorkspaceListingPrimary(
        kind="workspace_listing",
        entries=_entries(),
        graph_revision="rev_workspace_builder",
        data_classes=["workspace.file"],
        replayability="fully_managed",
    )


def _entries() -> list[WorkspaceListingEntry]:
    return [
        WorkspaceListingEntry(
            path="src/lib.rs",
            kind="file",
            data_classes=["workspace.file"],
            blob_ref=WireBlobRef(
                id="blob_workspace_entry",
                sha256="a" * 64,
                bytes=42,
                data_classes=["workspace.file"],
            ),
        )
    ]


def _snapshot_primary(digest: str) -> WorkspaceSnapshotPrimary:
    return WorkspaceSnapshotPrimary(
        kind="workspace_snapshot",
        workspace="ws_workspace_builder",
        digest=digest,
        graph_revision="rev_workspace_builder",
        data_classes=["workspace.file"],
        replayability="fully_managed",
    )


def _diff_primary(text: str) -> WorkspaceDiffPrimary:
    return WorkspaceDiffPrimary(
        kind="workspace_diff",
        graph_revision="rev_workspace_builder",
        data_classes=["workspace.file"],
        replayability="fully_managed",
        text=text,
    )


def _query_op(request: WorkspaceQueryRequest) -> JsonObject:
    params = _params_object(request.to_params())
    ops = _json_array(params["ops"])
    op = _json_object(ops[0])
    expr = _json_object(op["expr"])
    return _json_object(expr["op"])


def _params_object(params: object) -> JsonObject:
    value = json.loads(msgspec.json.encode(params))
    if not isinstance(value, dict):
        raise TypeError("expected JSON object")
    return value


def _json_array(value: JsonValue) -> list[JsonValue]:
    if not isinstance(value, list):
        raise TypeError("expected JSON array")
    return value


def _json_object(value: JsonValue) -> JsonObject:
    if not isinstance(value, dict):
        raise TypeError("expected JSON object")
    return value


__all__ = []
