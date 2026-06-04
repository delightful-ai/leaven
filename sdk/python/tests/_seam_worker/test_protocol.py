import io
import json
import sys

import pytest

from leaven._seam._wire.codec import encode_request
from leaven._seam._wire.payloads import RunnerRequest, StageRunRequest
from leaven._seam_worker.protocol import (
    WorkerProtocolError,
    WorkerRequest,
    read_request,
    write_error,
    write_result,
)


def test_read_request_accepts_one_stage_run_object(monkeypatch: pytest.MonkeyPatch) -> None:
    params = _runner_params()
    monkeypatch.setattr(
        sys,
        "stdin",
        io.StringIO(
            encode_request(method="leaven/stage.run", request_id="req_1", params=params).decode()
        ),
    )

    assert read_request() == WorkerRequest(request_id="req_1", params=params)


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


def test_read_request_rejects_untyped_stage_params(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setattr(
        sys,
        "stdin",
        io.StringIO(
            json.dumps(
                {
                    "jsonrpc": "2.0",
                    "method": "leaven/stage.run",
                    "id": "req_bad",
                    "params": {"payload": {"role": "runner"}},
                }
            )
        ),
    )

    with pytest.raises(WorkerProtocolError, match=r"invalid stage.run params"):
        read_request()


def test_write_result_preserves_json_rpc_id(capsys: pytest.CaptureFixture[str]) -> None:
    write_result(WorkerRequest(request_id=12, params=_runner_params()), {"ok": True})

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


def _runner_params() -> StageRunRequest:
    return StageRunRequest(
        schema_version="leaven.stage_run.v1",
        message="stage_run_request",
        stage="runner",
        payload=RunnerRequest(
            schema_version="leaven.stage_payloads.v1",
            run="run_worker_protocol",
            stage_call_id="sc_worker_protocol",
            candidate="cand_worker_protocol",
            case="case_worker_protocol",
            case_input={"prompt": "Say ok."},
            target_forbidden=True,
        ),
    )
