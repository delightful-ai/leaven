"""Private generated wire metadata and msgspec JSON-RPC codec for `_seam`."""

from .codec import decode_batch_responses, decode_response, decode_response_object, encode_request
from .errors import JsonRpcError, JsonRpcProtocolError, JsonRpcRemoteError
from .json_value import JsonArray, JsonObject, JsonRpcId, JsonScalar, JsonValue
from .methods import (
    LOCKED_METHODS,
    METHOD_BINDINGS,
    LockedMethod,
    LockedMethodBinding,
    require_locked_method,
)
from .payloads import (
    PLAN_RESULT_SCHEMA_FINGERPRINT,
    PLAN_SCHEMA_FINGERPRINT,
    STAGE_RUN_SCHEMA_FINGERPRINT,
    PlanDocument,
    PlanResultDocument,
    StageRunKind,
    StageRunRequest,
    StageRunResult,
)

__all__ = [
    "LOCKED_METHODS",
    "METHOD_BINDINGS",
    "PLAN_RESULT_SCHEMA_FINGERPRINT",
    "PLAN_SCHEMA_FINGERPRINT",
    "STAGE_RUN_SCHEMA_FINGERPRINT",
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
    "PlanDocument",
    "PlanResultDocument",
    "StageRunKind",
    "StageRunRequest",
    "StageRunResult",
    "decode_batch_responses",
    "decode_response",
    "decode_response_object",
    "encode_request",
    "require_locked_method",
]
