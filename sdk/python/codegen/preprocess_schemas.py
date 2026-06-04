"""Preprocess locked public-seam JSON Schemas for datamodel-code-generator.

JSON Schema 2020-12 permits boolean schema values: `true` (any value passes)
and `false` (no value passes). datamodel-code-generator (as of 0.34) does
not handle these; it expects every schema position to be a JsonSchemaObject
dict.

This script walks each schema with knowledge of which keyword values are
themselves schemas (vs other types like booleans or arrays), and normalizes
boolean-schema sentinels:

  - `True`  → `{}` (semantically equivalent: matches any value)
  - `False` → `{"not": {}}` (matches nothing)

Boolean-schema positions per JSON Schema 2020-12:

- `properties.*` — values are schemas
- `patternProperties.*` — values are schemas
- `dependentSchemas.*` — values are schemas
- `$defs.*`, `definitions.*` — values are schemas
- `items` — value is a schema (or list of schemas in draft-07)
- `prefixItems.*` — values are schemas
- `additionalProperties` — schema OR boolean (boolean is valid native, don't rewrite)
- `unevaluatedProperties` — schema OR boolean (don't rewrite)
- `additionalItems` — schema OR boolean (don't rewrite)
- `unevaluatedItems` — schema OR boolean (don't rewrite)
- `if`, `then`, `else` — schemas
- `not` — schema
- `contains`, `propertyNames` — schemas
- `allOf.*`, `anyOf.*`, `oneOf.*` — schemas

Usage:
    uv run python codegen/preprocess_schemas.py SOURCE_DIR DEST_DIR
"""

import json
import sys
from pathlib import Path

type JsonArray = list[JsonValue]
type JsonObject = dict[str, JsonValue]
type JsonScalar = str | int | float | bool | None
type JsonValue = JsonScalar | JsonObject | JsonArray

# Keys whose value is always a schema (recurse + normalize).
SCHEMA_VALUE_KEYS = {
    "items",
    "if",
    "then",
    "else",
    "not",
    "contains",
    "propertyNames",
    # `additionalProperties`, `additionalItems`, `unevaluatedProperties`,
    # `unevaluatedItems` are intentionally NOT in this set — the keyword
    # legitimately accepts a boolean, so we leave booleans alone there.
}

# Keys whose value is a dict-of-name-to-schema (recurse into each value).
SCHEMA_MAP_KEYS = {
    "properties",
    "patternProperties",
    "dependentSchemas",
    "$defs",
    "definitions",
}

# Keys whose value is a list-of-schemas (recurse into each element).
SCHEMA_LIST_KEYS = {
    "allOf",
    "anyOf",
    "oneOf",
    "prefixItems",
}


def normalize_schema(node: JsonValue) -> JsonValue:
    """Recursively normalize boolean schemas inside a schema position."""
    if node is True:
        return {}
    if node is False:
        return {"not": {}}
    if not isinstance(node, dict):
        return node
    out: JsonObject = {}
    for k, v in node.items():
        if k in SCHEMA_VALUE_KEYS:
            out[k] = normalize_schema(v)
        elif k in SCHEMA_MAP_KEYS and isinstance(v, dict):
            out[k] = {name: normalize_schema(sub) for name, sub in v.items()}
        elif k in SCHEMA_LIST_KEYS and isinstance(v, list):
            out[k] = [normalize_schema(sub) for sub in v]
        else:
            out[k] = v  # leave booleans/values alone outside schema positions
    return out


def json_value(value: object) -> JsonValue:
    """Return `value` as a JSON value or raise `TypeError`."""
    if value is None or isinstance(value, str | int | float | bool):
        return value
    if isinstance(value, dict):
        output: JsonObject = {}
        for key, item in value.items():
            if not isinstance(key, str):
                raise TypeError("JSON object keys must be strings")
            output[key] = json_value(item)
        return output
    if isinstance(value, list):
        return [json_value(item) for item in value]
    raise TypeError(f"value is not JSON: {type(value).__name__}")


def main(argv: list[str]) -> int:
    if len(argv) != 3:
        print(__doc__, file=sys.stderr)
        return 2

    src = Path(argv[1])
    dst = Path(argv[2])
    dst.mkdir(parents=True, exist_ok=True)

    schema_files = sorted(src.glob("*.schema.json"))
    if not schema_files:
        print(f"no *.schema.json found under {src}", file=sys.stderr)
        return 1

    for path in schema_files:
        with path.open() as f:
            schema = json_value(json.load(f))
        normalized = normalize_schema(schema)
        out = dst / path.name
        with out.open("w") as f:
            json.dump(normalized, f, indent=2, sort_keys=False)
            f.write("\n")
        print(f"  {path.name} -> {out}")

    print(f"normalized {len(schema_files)} schema(s) into {dst}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
