"""JSON-RPC protocol helpers for one command-runner worker request."""

import json
import sys
from collections.abc import Mapping

from .._seam._wire import JsonObject, JsonRpcId, JsonValue
from .._seam._wire.json_value import json_object


class WorkerProtocolError(RuntimeError):
    """The worker received or produced an invalid one-shot JSON-RPC message."""


def read_request() -> JsonObject:
    """Read one JSON-RPC request from stdin."""
    line = sys.stdin.readline()
    if not line:
        raise WorkerProtocolError("stdin closed before a stage.run request")
    request = json_object(json.loads(line))
    if request.get("method") != "leaven/stage.run":
        raise WorkerProtocolError(f"unexpected worker method: {request.get('method')!r}")
    return request


def write_result(request: Mapping[str, JsonValue], result: Mapping[str, JsonValue]) -> None:
    """Write one JSON-RPC result to stdout."""
    _write(json_object({"jsonrpc": "2.0", "id": request.get("id"), "result": dict(result)}))


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
