"""JSON value aliases for private public-seam wire records."""

from collections.abc import Mapping, Sequence

type JsonScalar = str | int | float | bool | None
type JsonValue = JsonScalar | dict[str, JsonValue] | list[JsonValue]
type JsonObject = dict[str, JsonValue]
type JsonArray = list[JsonValue]
type JsonRpcId = str | int | None


def json_value(raw_json: object) -> JsonValue:
    """Return `value` as a JSON value or raise `TypeError`."""
    if raw_json is None or isinstance(raw_json, str | int | float | bool):
        return raw_json
    if isinstance(raw_json, Mapping):
        output: JsonObject = {}
        for key, item in raw_json.items():
            if not isinstance(key, str):
                raise TypeError("JSON object keys must be strings")
            output[key] = json_value(item)
        return output
    if isinstance(raw_json, Sequence) and not isinstance(raw_json, str | bytes | bytearray):
        return [json_value(item) for item in raw_json]
    raise TypeError(f"value is not JSON-encodable: {type(raw_json).__name__}")


def json_object(raw_json: object) -> JsonObject:
    """Return `value` as a JSON object or raise `TypeError`."""
    parsed = json_value(raw_json)
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
