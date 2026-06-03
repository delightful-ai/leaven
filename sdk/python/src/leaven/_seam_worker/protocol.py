"""JSON-RPC protocol helpers for one command-runner worker request."""

from __future__ import annotations

import json
import sys
from collections.abc import Mapping
from typing import Any


class WorkerProtocolError(RuntimeError):
    """The worker received or produced an invalid one-shot JSON-RPC message."""


def read_request() -> dict[str, Any]:
    """Read one JSON-RPC request from stdin."""
    line = sys.stdin.readline()
    if not line:
        raise WorkerProtocolError("stdin closed before a stage.run request")
    request = json.loads(line)
    if request.get("method") != "leaven/stage.run":
        raise WorkerProtocolError(f"unexpected worker method: {request.get('method')!r}")
    return request


def write_result(request: Mapping[str, Any], result: Mapping[str, Any]) -> None:
    """Write one JSON-RPC result to stdout."""
    _write({"jsonrpc": "2.0", "id": request.get("id"), "result": result})


def write_error(request_id: object, message: str) -> None:
    """Write one JSON-RPC error to stdout."""
    _write(
        {
            "jsonrpc": "2.0",
            "id": request_id,
            "error": {
                "code": -32000,
                "message": message,
            },
        }
    )


def _write(message: Mapping[str, Any]) -> None:
    print(json.dumps(message, sort_keys=True), flush=True)
