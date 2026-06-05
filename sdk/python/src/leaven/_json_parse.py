"""Private JSON-shaped Python value parser for SDK boundary helpers."""

from collections.abc import Sequence

from .json_value import JsonArray, JsonObject, JsonValue


def parse_json_object(raw_json: object, *, context: str) -> JsonObject:
    """Parse an unknown Python value into a JSON object."""
    if not isinstance(raw_json, dict):
        raise TypeError(f"{context} must be a JSON object")
    output: JsonObject = {}
    for key, item in raw_json.items():
        if not isinstance(key, str):
            raise TypeError(f"{context} object keys must be strings")
        output[key] = parse_json_value(item, context=context)
    return output


def parse_json_value(raw_json: object, *, context: str) -> JsonValue:
    """Parse an unknown Python value into the SDK JSON value alias."""
    if raw_json is None or isinstance(raw_json, str | int | float | bool):
        return raw_json
    if isinstance(raw_json, dict):
        return parse_json_object(raw_json, context=context)
    if isinstance(raw_json, Sequence) and not isinstance(raw_json, str | bytes | bytearray):
        return _parse_json_array(raw_json, context=context)
    raise TypeError(f"{context} contains non-JSON value: {type(raw_json).__name__}")


def _parse_json_array(raw_json: Sequence[object], *, context: str) -> JsonArray:
    return [parse_json_value(item, context=context) for item in raw_json]


__all__ = ["parse_json_object", "parse_json_value"]
