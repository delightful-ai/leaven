"""Private lowering helpers for SDK output contracts."""

import hashlib
import json
import math

from ..json_value import JsonSchema, JsonValue


def schema_fingerprint(schema: JsonSchema) -> str:
    """Return the public-seam schema fingerprint for a JSON Schema object."""
    canonical = _canonical_json(schema)
    return f"fp_schema_sha256_{hashlib.sha256(canonical.encode()).hexdigest()}"


def _canonical_json(value: JsonValue) -> str:
    if value is None:
        canonical = "null"
    elif isinstance(value, bool):
        canonical = "true" if value else "false"
    elif isinstance(value, str):
        canonical = json.dumps(value, ensure_ascii=False, separators=(",", ":"))
    elif isinstance(value, int):
        canonical = _canonical_int(value)
    elif isinstance(value, float):
        canonical = _canonical_float(value)
    elif isinstance(value, list):
        canonical = "[" + ",".join(_canonical_json(item) for item in value) + "]"
    else:
        canonical = "{" + ",".join(
            f"{_canonical_json(key)}:{_canonical_json(value[key])}" for key in sorted(value)
        ) + "}"
    return canonical


def _canonical_float(value: float) -> str:
    if not math.isfinite(value):
        raise ValueError("JSON schema numbers must be finite")
    if value == 0:
        return "0"
    text = json.dumps(value, allow_nan=False, separators=(",", ":"))
    if "e" in text:
        mantissa, exponent = text.split("e", 1)
        return f"{mantissa}E{_canonical_exponent(exponent)}"
    return text


def _canonical_int(number: int) -> str:
    return f"{number}"


def _canonical_exponent(exponent: str) -> str:
    sign = ""
    digits = exponent
    if exponent.startswith("-"):
        sign = "-"
        digits = exponent[1:]
    elif exponent.startswith("+"):
        digits = exponent[1:]
    digits = digits.lstrip("0") or "0"
    return sign + digits


__all__ = ["schema_fingerprint"]
