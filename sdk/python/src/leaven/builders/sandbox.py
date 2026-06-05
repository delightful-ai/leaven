"""`cx.sandbox.*` — sandboxed command execution with typed output capture."""

import asyncio
import math
from collections.abc import Sequence
from typing import Literal, Protocol

from msgspec import UNSET, UnsetType
from pydantic import BaseModel, ConfigDict

from .._handles import WorkspaceHandle
from .._receipts import CallReceipt
from .._seam import SandboxExecRequest
from .._seam._wire.payloads import Cost
from .._seam._wire.refs import BlobRef as WireBlobRef
from .._seam._wire.results import SandboxExecResult
from ..blob_ref import BlobRef
from ..json_value import JsonObject
from ..output import (
    FilesOutput,
    JsonSchemaOutput,
    JsonSchemaValueOutput,
    OutputContract,
    TextOutput,
)
from ._output_contract import json_schema_output_to_wire


class SandboxExec(BaseModel):
    """Result of `cx.sandbox.exec(...)`."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    exit_code: int
    stdout_ref: BlobRef
    """Blob ref for stdout bytes (always captured when stream_policy=blob_refs_only)."""
    stderr_ref: BlobRef
    files: dict[str, BlobRef] | None = None
    """Captured output file blob refs when `output=lv.output.files(...)`."""
    cost_usd: float | None = None
    receipt: CallReceipt


StreamPolicy = Literal["blob_refs_only", "live_stream"]


class _SeamRequester(Protocol):
    """Small private protocol SandboxBuilder needs from the seam client."""

    def sandbox_exec(self, request: SandboxExecRequest) -> SandboxExecResult: ...


class SandboxBuilder:
    """Sandboxed exec bound to a context. Requires a materialized workspace."""

    def __init__(
        self,
        *,
        _client: "_SeamRequester | None" = None,
        _idempotency_prefix: str = "sandbox-builder",
        _plan_id: str = "planpythonsandboxbuilder001",
    ) -> None:
        self._client = _client
        self._idempotency_prefix = _idempotency_prefix
        self._plan_id = _plan_id

    @classmethod
    def _for_seam(
        cls,
        client: "_SeamRequester",
        *,
        idempotency_prefix: str = "sandbox-builder",
        plan_id: str = "planpythonsandboxbuilder001",
    ) -> "SandboxBuilder":
        """Bind this builder to the private public-seam process client."""
        return cls(
            _client=client,
            _idempotency_prefix=idempotency_prefix,
            _plan_id=plan_id,
        )

    async def exec(
        self,
        *,
        workspace: WorkspaceHandle,
        argv: Sequence[str],
        env: dict[str, str] | None = None,
        cwd: str | None = None,
        timeout_s: float | None = None,
        output: OutputContract | None = None,
        stream_policy: StreamPolicy = "blob_refs_only",
        input_classes: Sequence[str] | None = None,
        forbidden_input_classes: Sequence[str] | None = None,
    ) -> SandboxExec:
        """Run a command in the configured sandbox against the workspace.

        Sandbox configuration comes from the runtime's `sandbox=` slot.
        Output captures are bound to the receipt; the engine refuses captures
        outside the contract.
        """
        if self._client is None:
            raise NotImplementedError(
                "SandboxBuilder.exec needs an engine-bound public-seam client; "
                "use the cx.sandbox instance supplied to a running stage"
            )
        request = SandboxExecRequest(
            request_id=f"{self._idempotency_prefix}-exec",
            plan_id=self._plan_id,
            idempotency_key=f"{self._idempotency_prefix}-exec",
            workspace=workspace.workspace_id,
            argv=list(argv),
            env=env,
            cwd=cwd,
            timeout_s=_timeout_seconds(timeout_s),
            output=_output_to_wire(output),
            stream_policy=_stream_policy_to_wire(stream_policy),
            input_classes=input_classes,
            forbidden_input_classes=forbidden_input_classes,
        )
        result = await asyncio.to_thread(self._client.sandbox_exec, request)
        return _sandbox_exec_from_result(result)


def _sandbox_exec_from_result(result: SandboxExecResult) -> SandboxExec:
    primary = result.primary
    stdout = _required_blob_ref(primary.stdout_ref, "stdout_ref")
    stderr = _required_blob_ref(primary.stderr_ref, "stderr_ref")
    files = _file_refs(primary.files)
    return SandboxExec(
        exit_code=_required_exit_code(primary.exit_code),
        stdout_ref=stdout,
        stderr_ref=stderr,
        files=files,
        cost_usd=_cost_usd(primary.cost),
        receipt=CallReceipt(
            receipt_id=primary.receipt,
            blob_refs=[stdout, stderr, *([] if files is None else files.values())],
        ),
    )


def _output_to_wire(output: OutputContract | None) -> JsonObject:
    if output is None:
        return {"kind": "files", "paths": []}
    if isinstance(output, FilesOutput):
        value: JsonObject = {"kind": "files", "paths": list(output.paths)}
        if output.max_bytes is not None:
            value["max_bytes"] = output.max_bytes
        return value
    if isinstance(output, TextOutput):
        value: JsonObject = {"kind": "final_message"}
        if output.max_chars is not None:
            value["max_bytes"] = output.max_chars
        return value
    if isinstance(output, JsonSchemaOutput | JsonSchemaValueOutput):
        return json_schema_output_to_wire(output)
    raise TypeError(f"unsupported sandbox output contract: {type(output).__name__}")


def _stream_policy_to_wire(
    stream_policy: StreamPolicy,
) -> Literal["stream_updates", "blob_refs_only"]:
    if stream_policy == "blob_refs_only":
        return "blob_refs_only"
    if stream_policy == "live_stream":
        return "stream_updates"
    raise ValueError("unsupported sandbox stream policy")


def _timeout_seconds(timeout_s: float | None) -> int:
    return max(1, math.ceil(timeout_s)) if timeout_s is not None else 60


def _required_blob_ref(value: WireBlobRef | UnsetType, field: str) -> BlobRef:
    if value is UNSET:
        raise TypeError(f"sandbox result is missing {field}")
    return BlobRef(
        blob_id=value.id,
        sha256=value.sha256,
        bytes=value.bytes,
        data_classes=list(value.data_classes),
    )


def _file_refs(value: dict[str, WireBlobRef] | UnsetType) -> dict[str, BlobRef] | None:
    if value is UNSET:
        return None
    return {path: _required_blob_ref(blob, f"files[{path!r}]") for path, blob in value.items()}


def _required_exit_code(value: int | UnsetType) -> int:
    if value is UNSET:
        raise TypeError("sandbox result is missing exit_code")
    return value


def _cost_usd(cost: Cost) -> float | None:
    usd_micro = cost.usd_micro
    return None if usd_micro is UNSET else usd_micro / 1_000_000


__all__ = ["SandboxBuilder", "SandboxExec", "StreamPolicy"]
