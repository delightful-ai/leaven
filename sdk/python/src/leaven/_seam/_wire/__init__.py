"""Private generated wire metadata and msgspec JSON-RPC codec for `_seam`."""

from .codec import decode_batch_responses, decode_response, decode_response_object, encode_request
from .errors import JsonRpcError, JsonRpcProtocolError, JsonRpcRemoteError
from .json_value import JsonArray, JsonObject, JsonRpcId, JsonScalar, JsonValue
from .methods import LOCKED_METHODS, METHOD_BINDINGS, LockedMethod, LockedMethodBinding

__all__ = [
    "LOCKED_METHODS",
    "METHOD_BINDINGS",
    "JsonArray",
    "JsonObject",
    "JsonRpcError",
    "JsonRpcId",
    "JsonRpcProtocolError",
    "JsonRpcRemoteError",
    "JsonScalar",
    "JsonValue",
    "LockedMethod",
    "LockedMethodBinding",
    "decode_batch_responses",
    "decode_response",
    "decode_response_object",
    "encode_request",
]
