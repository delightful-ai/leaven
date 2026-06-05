"""JSON-RPC protocol helpers for one command-runner worker request."""

import sys
from dataclasses import dataclass
from typing import Literal

import msgspec
from msgspec import UNSET, Struct

from .._seam._wire import JsonRpcId
from .._seam._wire.errors import JsonRpcError
from .._seam._wire.jsonrpc import JsonRpcRequestEnvelope
from .._seam._wire.payloads import StageRunRequest, StageRunResult


class WorkerProtocolError(RuntimeError):
    """The worker received or produced an invalid one-shot JSON-RPC message."""


@dataclass(frozen=True)
class WorkerRequest:
    """A typed worker request decoded from one JSON-RPC line."""

    request_id: JsonRpcId
    params: StageRunRequest


class WorkerSuccessResponse(Struct, frozen=True, forbid_unknown_fields=True):
    """Typed JSON-RPC success response emitted by the worker."""

    id: JsonRpcId
    result: StageRunResult
    jsonrpc: Literal["2.0"] = "2.0"


class WorkerErrorResponse(Struct, frozen=True, forbid_unknown_fields=True):
    """Typed JSON-RPC error response emitted by the worker."""

    id: JsonRpcId
    error: JsonRpcError
    jsonrpc: Literal["2.0"] = "2.0"


_REQUEST_DECODER = msgspec.json.Decoder(JsonRpcRequestEnvelope)
_RESPONSE_ENCODER = msgspec.json.Encoder()


def read_request() -> WorkerRequest:
    """Read one JSON-RPC request from stdin."""
    line = sys.stdin.readline()
    if not line:
        raise WorkerProtocolError("stdin closed before a stage.run request")
    try:
        envelope = _REQUEST_DECODER.decode(line.encode())
    except msgspec.DecodeError as error:
        raise WorkerProtocolError(f"invalid JSON-RPC request: {error}") from error
    if envelope.method != "leaven/stage.run":
        raise WorkerProtocolError(f"unexpected worker method: {envelope.method!r}")
    if envelope.id is UNSET:
        raise WorkerProtocolError("stage.run worker request must include a JSON-RPC id")
    if envelope.params is UNSET:
        raise WorkerProtocolError("stage.run worker request must include params")
    try:
        params = msgspec.json.decode(envelope.params, type=StageRunRequest)
    except msgspec.DecodeError as error:
        raise WorkerProtocolError(f"invalid stage.run params: {error}") from error
    return WorkerRequest(request_id=envelope.id, params=params)


def write_result(request: WorkerRequest, result: StageRunResult) -> None:
    """Write one JSON-RPC result to stdout."""
    _write(WorkerSuccessResponse(id=request.request_id, result=result))


def write_error(request_id: JsonRpcId, message: str) -> None:
    """Write one JSON-RPC error to stdout."""
    _write(WorkerErrorResponse(id=request_id, error=JsonRpcError(code=-32000, message=message)))


def _write(message: WorkerSuccessResponse | WorkerErrorResponse) -> None:
    print(_RESPONSE_ENCODER.encode(message).decode(), flush=True)
