"""JSON-RPC 2.0 envelope records for Leaven's private seam client."""

from typing import Literal

from msgspec import UNSET, Raw, Struct, UnsetType

from .errors import JsonRpcError
from .json_value import JsonRpcId


class JsonRpcRequestEnvelope(Struct, frozen=True):
    """A JSON-RPC request or notification envelope."""

    method: str
    jsonrpc: Literal["2.0"] = "2.0"
    params: Raw | UnsetType = UNSET
    id: JsonRpcId | UnsetType = UNSET


class JsonRpcResponseEnvelope(Struct, frozen=True, omit_defaults=True):
    """A JSON-RPC response envelope parsed before method-specific result decode."""

    jsonrpc: Literal["2.0"]
    id: JsonRpcId
    result: Raw | UnsetType = UNSET
    error: JsonRpcError | UnsetType = UNSET


__all__ = ["JsonRpcRequestEnvelope", "JsonRpcResponseEnvelope"]
