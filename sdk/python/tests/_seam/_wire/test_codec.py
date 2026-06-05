"""Tests for `leaven._seam._wire.codec`."""

import json
from typing import cast

import msgspec
import pytest

from leaven._seam._wire import (
    LOCKED_METHODS,
    JsonRpcProtocolError,
    JsonRpcRemoteError,
    LockedMethod,
    decode_batch_responses,
    decode_method_response,
    decode_response,
    encode_request,
    method_result_type,
)
from leaven._seam._wire.errors import JsonRpcError
from leaven._seam._wire.jsonrpc import JsonRpcRequestEnvelope, JsonRpcResponseEnvelope
from leaven._seam._wire.method_results import METHOD_RESULT_TYPES
from leaven._seam._wire.methods import LockedMethodBinding
from leaven._seam._wire.payloads import (
    CommitPolicyNoGraphWrites,
    ConsistencyLatestAtStart,
    EvalModeExecute,
    LeavenValue,
    OperationReceipt,
    PlanDocument,
)
from leaven._seam._wire.results import (
    AgentRunResult,
    LmCompleteResult,
    MethodResultBinding,
    ResultReceipt,
)


class Widget(msgspec.Struct, frozen=True):
    """Typed result fixture."""

    ok: bool
    name: str


def plan_params() -> PlanDocument:
    """Return a minimal typed Plan IR params object."""
    return PlanDocument(
        schema_version="leaven.plan.v1",
        plan_id="plan_codec",
        consistency=ConsistencyLatestAtStart(),
        mode=EvalModeExecute(),
        ops=[],
        return_=[],
        commit=CommitPolicyNoGraphWrites(),
    )


def test_encode_request_accepts_locked_method() -> None:
    body = encode_request(
        method="leaven/lm.complete",
        request_id="req_1",
        params=plan_params(),
    )

    assert json.loads(body) == {
        "method": "leaven/lm.complete",
        "jsonrpc": "2.0",
        "params": {
            "schema_version": "leaven.plan.v1",
            "plan_id": "plan_codec",
            "consistency": {"kind": "latest_at_start"},
            "mode": {"kind": "execute"},
            "ops": [],
            "return": [],
            "commit": {"kind": "no_graph_writes"},
        },
        "id": "req_1",
    }


def test_encode_request_omits_notification_id() -> None:
    body = encode_request(
        method="leaven/event.emit",
        request_id=msgspec.UNSET,
        params=plan_params(),
    )

    assert "id" not in json.loads(body)


def test_encode_request_rejects_unknown_method() -> None:
    method = cast("LockedMethod", "leaven/not_a_locked_method")
    with pytest.raises(ValueError, match="unknown locked Leaven public-seam method"):
        encode_request(method=method, request_id="req_1", params=plan_params())


def test_boundary_structs_forbid_unknown_fields() -> None:
    strict_records: tuple[type[msgspec.Struct], ...] = (
        JsonRpcRequestEnvelope,
        JsonRpcResponseEnvelope,
        JsonRpcError,
        LockedMethodBinding,
        MethodResultBinding,
        LeavenValue,
        OperationReceipt,
        ResultReceipt,
    )

    for record_type in strict_records:
        assert record_type.__struct_config__.forbid_unknown_fields


def test_decode_request_envelope_rejects_unknown_fields() -> None:
    body = (
        b'{"jsonrpc":"2.0","method":"leaven/lm.complete","id":"req_1",'
        b'"unexpected":true}'
    )

    with pytest.raises(msgspec.ValidationError, match="unexpected"):
        msgspec.json.decode(body, type=JsonRpcRequestEnvelope)


def test_decode_response_decodes_method_specific_raw_result() -> None:
    body = b'{"jsonrpc":"2.0","id":"req_1","result":{"ok":true,"name":"done"}}'

    assert decode_response(body, Widget) == Widget(ok=True, name="done")


def test_method_result_types_cover_every_locked_method() -> None:
    assert set(METHOD_RESULT_TYPES) == set(LOCKED_METHODS)


def test_decode_method_response_uses_locked_method_result_type() -> None:
    body = (
        b'{"jsonrpc":"2.0","id":"req_1","result":{"method":"leaven/lm.complete",'
        b'"primary":{"kind":"lm_response","message":{"role":"assistant","content":'
        b'[{"kind":"text","text":"ok"}]},"receipt":"lmrec_1","graph_revision":"rev",'
        b'"data_classes":["public"],"replayability":"boundary_managed"},"receipts":[],'
        b'"redactions":[],"capability_fingerprint":"fp_cap",'
        b'"policy_fingerprint":"fp_policy","data_classes":["public"]}}'
    )

    decoded = decode_method_response(body, "leaven/lm.complete")

    assert isinstance(decoded, LmCompleteResult)
    assert decoded.primary.message.content[0].text == "ok"
    assert method_result_type("leaven/lm.complete") is LmCompleteResult


def test_decode_method_response_rejects_mismatched_method_result_shape() -> None:
    body = (
        b'{"jsonrpc":"2.0","id":"req_1","result":{"method":"leaven/lm.complete",'
        b'"primary":{"kind":"lm_response","message":{"role":"assistant","content":'
        b'[{"kind":"text","text":"ok"}]},"receipt":"lmrec_1","graph_revision":"rev",'
        b'"data_classes":["public"],"replayability":"boundary_managed"},"receipts":[],'
        b'"redactions":[],"capability_fingerprint":"fp_cap",'
        b'"policy_fingerprint":"fp_policy","data_classes":["public"]}}'
    )

    assert method_result_type("leaven/agent.run") is AgentRunResult
    with pytest.raises(JsonRpcProtocolError):
        decode_method_response(body, "leaven/agent.run")


def test_decode_response_allows_null_id() -> None:
    body = b'{"jsonrpc":"2.0","id":null,"result":{"ok":true,"name":"done"}}'

    assert decode_response(body, Widget) == Widget(ok=True, name="done")


def test_decode_response_rejects_both_result_and_error() -> None:
    body = (
        b'{"jsonrpc":"2.0","id":"req_1","result":{"ok":true,"name":"done"},'
        b'"error":{"code":-32000,"message":"no"}}'
    )

    with pytest.raises(JsonRpcProtocolError, match="exactly one"):
        decode_response(body, Widget)


def test_decode_response_rejects_neither_result_nor_error() -> None:
    body = b'{"jsonrpc":"2.0","id":"req_1"}'

    with pytest.raises(JsonRpcProtocolError, match="exactly one"):
        decode_response(body, Widget)


def test_decode_response_rejects_malformed_envelope() -> None:
    body = b'{"jsonrpc":"1.0","id":"req_1","result":{"ok":true,"name":"done"}}'

    with pytest.raises(JsonRpcProtocolError):
        decode_response(body, Widget)


def test_decode_response_rejects_unknown_envelope_fields() -> None:
    body = (
        b'{"jsonrpc":"2.0","id":"req_1","result":{"ok":true,"name":"done"},'
        b'"unexpected":true}'
    )

    with pytest.raises(JsonRpcProtocolError, match="unexpected"):
        decode_response(body, Widget)


def test_decode_error_rejects_unknown_fields() -> None:
    body = (
        b'{"jsonrpc":"2.0","id":"req_1","error":{"code":-32000,'
        b'"message":"no","unexpected":true}}'
    )

    with pytest.raises(JsonRpcProtocolError, match="unexpected"):
        decode_response(body, Widget)


def test_decode_error_rejects_untyped_error_data() -> None:
    body = (
        b'{"jsonrpc":"2.0","id":"req_1","error":{"code":-32000,'
        b'"message":"no","data":{"debug":"raw"}}}'
    )

    with pytest.raises(JsonRpcProtocolError, match="unknown field `data`"):
        decode_response(body, Widget)


def test_decode_response_raises_remote_error() -> None:
    body = b'{"jsonrpc":"2.0","id":"req_1","error":{"code":-32000,"message":"no"}}'

    with pytest.raises(JsonRpcRemoteError, match="JSON-RPC error -32000: no"):
        decode_response(body, Widget)


def test_decode_response_rejects_typed_result_mismatch() -> None:
    body = b'{"jsonrpc":"2.0","id":"req_1","result":{"ok":"yes","name":"done"}}'

    with pytest.raises(JsonRpcProtocolError):
        decode_response(body, Widget)


def test_decode_batch_routes_by_id_not_response_order() -> None:
    body = (
        b'[{"jsonrpc":"2.0","id":2,"result":{"ok":true,"name":"second"}},'
        b'{"jsonrpc":"2.0","id":1,"result":{"ok":true,"name":"first"}}]'
    )

    decoded = decode_batch_responses(body, expected={1: Widget, 2: Widget})

    assert decoded == {
        1: Widget(ok=True, name="first"),
        2: Widget(ok=True, name="second"),
    }


def test_decode_batch_routes_null_id_when_expected() -> None:
    body = b'[{"jsonrpc":"2.0","id":null,"result":{"ok":true,"name":"done"}}]'

    assert decode_batch_responses(body, expected={None: Widget}) == {
        None: Widget(ok=True, name="done")
    }


def test_decode_batch_rejects_missing_expected_id() -> None:
    body = b'[{"jsonrpc":"2.0","id":1,"result":{"ok":true,"name":"done"}}]'

    with pytest.raises(JsonRpcProtocolError, match="missing JSON-RPC response ids"):
        decode_batch_responses(body, expected={1: Widget, 2: Widget})
