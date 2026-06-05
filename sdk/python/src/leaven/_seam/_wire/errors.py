"""Exceptions raised by the private public-seam wire codec."""

from msgspec import UNSET, Raw, Struct, UnsetType


class JsonRpcProtocolError(ValueError):
    """The local or remote peer sent an invalid JSON-RPC envelope."""


class JsonRpcError(Struct, frozen=True, forbid_unknown_fields=True, omit_defaults=True):
    """A JSON-RPC error object with raw optional extension data."""

    code: int
    message: str
    data: Raw | UnsetType = UNSET


class JsonRpcRemoteError(RuntimeError):
    """The remote peer returned a JSON-RPC error response."""

    def __init__(self, error: JsonRpcError) -> None:
        self.error = error
        super().__init__(f"JSON-RPC error {error.code}: {error.message}")


__all__ = ["JsonRpcError", "JsonRpcProtocolError", "JsonRpcRemoteError"]
