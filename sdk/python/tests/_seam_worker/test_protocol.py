import io
import json
import sys

import pytest

from leaven._seam_worker.protocol import (
    WorkerProtocolError,
    read_request,
    write_error,
    write_result,
)


def test_read_request_accepts_one_stage_run_object(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(
        sys,
        "stdin",
        io.StringIO(
            json.dumps(
                {
                    "jsonrpc": "2.0",
                    "method": "leaven/stage.run",
                    "id": "req_1",
                    "params": {"payload": {"role": "runner"}},
                }
            )
        ),
    )

    assert read_request() == {
        "jsonrpc": "2.0",
        "method": "leaven/stage.run",
        "id": "req_1",
        "params": {"payload": {"role": "runner"}},
    }


def test_read_request_rejects_unexpected_method(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    monkeypatch.setattr(
        sys,
        "stdin",
        io.StringIO(json.dumps({"jsonrpc": "2.0", "method": "leaven/lm.complete"})),
    )

    with pytest.raises(WorkerProtocolError, match="unexpected worker method"):
        read_request()


def test_write_result_preserves_json_rpc_id(capsys: pytest.CaptureFixture[str]) -> None:
    write_result({"id": 12}, {"ok": True})

    assert json.loads(capsys.readouterr().out) == {
        "jsonrpc": "2.0",
        "id": 12,
        "result": {"ok": True},
    }


def test_write_error_uses_worker_error_code(capsys: pytest.CaptureFixture[str]) -> None:
    write_error("req_2", "bad stage")

    assert json.loads(capsys.readouterr().out) == {
        "jsonrpc": "2.0",
        "id": "req_2",
        "error": {"code": -32000, "message": "bad stage"},
    }
