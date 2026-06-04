"""JSON value aliases for private public-seam wire records."""

from collections.abc import Mapping, Sequence

type JsonScalar = str | int | float | bool | None
type JsonValue = JsonScalar | dict[str, JsonValue] | list[JsonValue]
type JsonObject = dict[str, JsonValue]
type JsonArray = list[JsonValue]
type JsonRpcId = str | int | None


def json_value(value: object) -> JsonValue:
    """Return `value` as a JSON value or raise `TypeError`."""
    if value is None or isinstance(value, str | int | float | bool):
        return value
    if isinstance(value, Mapping):
        output: JsonObject = {}
        for key, item in value.items():
            if not isinstance(key, str):
                raise TypeError("JSON object keys must be strings")
            output[key] = json_value(item)
        return output
    if isinstance(value, Sequence) and not isinstance(value, str | bytes | bytearray):
        return [json_value(item) for item in value]
    raise TypeError(f"value is not JSON-encodable: {type(value).__name__}")


def json_object(value: object) -> JsonObject:
    """Return `value` as a JSON object or raise `TypeError`."""
    parsed = json_value(value)
    if not isinstance(parsed, dict):
        raise TypeError("JSON value must be an object")
    return parsed


__all__ = [
    "JsonArray",
    "JsonObject",
    "JsonRpcId",
    "JsonScalar",
    "JsonValue",
    "json_object",
    "json_value",
]
