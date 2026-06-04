"""msgspec JSON-RPC codec for the private public-seam client."""

from collections.abc import Mapping
from typing import cast

import msgspec
from msgspec import UNSET, Raw, UnsetType

from .errors import JsonRpcProtocolError, JsonRpcRemoteError
from .json_value import JsonRpcId
from .jsonrpc import JsonRpcRequestEnvelope, JsonRpcResponseEnvelope
from .methods import LockedMethod, require_locked_method
from .payloads import PlanDocument
from .payloads import StageRunRequest as StageRunDispatchRequest

type RequestParams = PlanDocument | StageRunDispatchRequest

_ENCODER = msgspec.json.Encoder()
_RESPONSE_DECODER = msgspec.json.Decoder(JsonRpcResponseEnvelope)
_BATCH_RESPONSE_DECODER = msgspec.json.Decoder(list[JsonRpcResponseEnvelope])


def encode_request(
    *,
    method: LockedMethod,
    request_id: JsonRpcId | UnsetType,
    params: RequestParams | Raw | UnsetType = UNSET,
) -> bytes:
    """Encode one locked-method JSON-RPC request or notification."""
    require_locked_method(method)
    envelope = JsonRpcRequestEnvelope(
        method=method,
        params=_raw_params(params),
        id=request_id,
    )
    return _ENCODER.encode(envelope)


def decode_response[T](body: bytes, result_type: type[T]) -> T:
    """Decode one JSON-RPC response and then decode its method-specific result."""
    try:
        envelope = _RESPONSE_DECODER.decode(body)
    except msgspec.DecodeError as error:
        raise JsonRpcProtocolError(str(error)) from error
    return _decode_envelope_result(envelope, result_type)


def decode_batch_responses[T](
    body: bytes,
    *,
    expected: Mapping[JsonRpcId, type[T]],
) -> dict[JsonRpcId, T]:
    """Decode a JSON-RPC response batch by id, never by response order."""
    try:
        envelopes = _BATCH_RESPONSE_DECODER.decode(body)
    except msgspec.DecodeError as error:
        raise JsonRpcProtocolError(str(error)) from error

    decoded: dict[JsonRpcId, T] = {}
    for envelope in envelopes:
        if envelope.id not in expected:
            raise JsonRpcProtocolError(f"unexpected JSON-RPC response id: {envelope.id!r}")
        if envelope.id in decoded:
            raise JsonRpcProtocolError(f"duplicate JSON-RPC response id: {envelope.id!r}")
        decoded[envelope.id] = _decode_envelope_result(envelope, expected[envelope.id])

    missing = set(expected) - set(decoded)
    if missing:
        raise JsonRpcProtocolError(f"missing JSON-RPC response ids: {sorted(missing, key=str)!r}")
    return decoded


def _raw_params(params: RequestParams | Raw | UnsetType) -> Raw | UnsetType:
    if params is UNSET:
        return UNSET
    if isinstance(params, Raw):
        return params
    return Raw(_ENCODER.encode(params))


def _decode_envelope_result[T](envelope: JsonRpcResponseEnvelope, result_type: type[T]) -> T:
    raw = _envelope_result_raw(envelope)
    try:
        return msgspec.json.decode(raw, type=result_type)
    except msgspec.DecodeError as error:
        raise JsonRpcProtocolError(str(error)) from error


def _envelope_result_raw(envelope: JsonRpcResponseEnvelope) -> Raw:
    has_result = envelope.result is not UNSET
    has_error = envelope.error is not UNSET
    if has_result == has_error:
        raise JsonRpcProtocolError("JSON-RPC response must contain exactly one of result or error")
    if has_error:
        raise JsonRpcRemoteError(envelope.error)
    return cast("Raw", envelope.result)


__all__ = ["RequestParams", "decode_batch_responses", "decode_response", "encode_request"]
