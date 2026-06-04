"""JSON-RPC protocol helpers for one command-runner worker request."""

import json
import sys
from collections.abc import Mapping
from dataclasses import dataclass

import msgspec
from msgspec import UNSET

from .._seam._wire import JsonRpcId, JsonValue
from .._seam._wire.json_value import json_object
from .._seam._wire.jsonrpc import JsonRpcRequestEnvelope
from .._seam._wire.payloads import StageRunRequest


class WorkerProtocolError(RuntimeError):
    """The worker received or produced an invalid one-shot JSON-RPC message."""


@dataclass(frozen=True)
class WorkerRequest:
    """A typed worker request decoded from one JSON-RPC line."""

    request_id: JsonRpcId
    params: StageRunRequest


_REQUEST_DECODER = msgspec.json.Decoder(JsonRpcRequestEnvelope)


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


def write_result(request: WorkerRequest, result: Mapping[str, JsonValue]) -> None:
    """Write one JSON-RPC result to stdout."""
    _write(json_object({"jsonrpc": "2.0", "id": request.request_id, "result": dict(result)}))


def write_error(request_id: JsonRpcId, message: str) -> None:
    """Write one JSON-RPC error to stdout."""
    _write(
        json_object(
            {
                "jsonrpc": "2.0",
                "id": request_id,
                "error": {
                    "code": -32000,
                    "message": message,
                },
            }
        )
    )


def _write(message: Mapping[str, JsonValue]) -> None:
    print(json.dumps(message, sort_keys=True), flush=True)
