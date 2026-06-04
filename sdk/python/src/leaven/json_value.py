"""JSON value aliases for public SDK records.

Public Leaven records carry JSON-shaped metadata, payloads, and schema
documents at several boundaries. These aliases keep that surface typed without
depending on the private `_seam._wire` codec package.
"""

from pydantic import JsonValue

type JsonArray = list[JsonValue]
type JsonObject = dict[str, JsonValue]
type JsonSchema = JsonObject

__all__ = ["JsonArray", "JsonObject", "JsonSchema", "JsonValue"]
