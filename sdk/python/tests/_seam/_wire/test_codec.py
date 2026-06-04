"""Tests for `leaven._seam._wire.codec`."""

import json

import msgspec
import pytest

from leaven._seam._wire import (
    JsonObject,
    JsonRpcProtocolError,
    JsonRpcRemoteError,
    decode_batch_responses,
    decode_response,
    encode_request,
)


class Widget(msgspec.Struct, frozen=True):
    """Typed result fixture."""

    ok: bool
    name: str


def plan_params() -> JsonObject:
    """Return a minimal Plan IR-shaped params object."""
    return {"schema_version": "leaven.plan.v1"}


def test_encode_request_accepts_locked_method() -> None:
    body = encode_request(
        method="leaven/lm.complete",
        request_id="req_1",
        params=plan_params(),
    )

    assert json.loads(body) == {
        "method": "leaven/lm.complete",
        "jsonrpc": "2.0",
        "params": {"schema_version": "leaven.plan.v1"},
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
    with pytest.raises(ValueError, match="unknown locked Leaven public-seam method"):
        encode_request(method="leaven/human.review", request_id="req_1", params={})


def test_decode_response_decodes_method_specific_raw_result() -> None:
    body = b'{"jsonrpc":"2.0","id":"req_1","result":{"ok":true,"name":"done"}}'

    assert decode_response(body, Widget) == Widget(ok=True, name="done")


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
