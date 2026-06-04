"""Built-in scoring helpers — common math users would otherwise rewrite.

Use directly (`lv.scoring.exact_match(...)`) or compose into a `@lv.reward`.
Not a complete catalog; rewards can do anything via `@lv.reward`.
"""

import math
from collections import Counter
from collections.abc import Sequence


def exact_match(output: str, target: str) -> float:
    """1.0 iff `output == target` after default normalization; else 0.0."""
    return normalized_match(output, target)


def normalized_match(
    output: str, target: str, *, lowercase: bool = True, strip: bool = True
) -> float:
    """Exact match after optional lowercase/strip normalization."""
    return 1.0 if _normalize(output, lowercase=lowercase, strip=strip) == _normalize(
        target,
        lowercase=lowercase,
        strip=strip,
    ) else 0.0


def multi_tolerance(
    output: str,
    target: str,
    *,
    tolerances: Sequence[float] = (0.0, 0.01, 0.025, 0.05, 0.10),
) -> float:
    """EvoSkill-style multi-tolerance numeric scorer.

    Parses both as floats; emits weighted score based on relative error
    against the tolerance ladder. Returns 0.0 if either side isn't numeric.
    """
    parsed_output = _parse_float(output)
    parsed_target = _parse_float(target)
    if parsed_output is None or parsed_target is None:
        return 0.0
    if parsed_output == parsed_target:
        return 1.0
    if parsed_target == 0.0:
        return 0.0
    tolerance_values = list(tolerances)
    if not tolerance_values:
        return 0.0
    relative_error = abs(parsed_output - parsed_target) / abs(parsed_target)
    passed = sum(1 for tolerance in tolerance_values if relative_error <= tolerance + 1e-12)
    return passed / len(tolerance_values)


def f1(output: str, target: str) -> float:
    """Token-level F1 between output and target."""
    output_tokens = _tokens(output)
    target_tokens = _tokens(target)
    if not output_tokens or not target_tokens:
        return 1.0 if output_tokens == target_tokens else 0.0
    overlap = sum((Counter(output_tokens) & Counter(target_tokens)).values())
    if overlap == 0:
        return 0.0
    precision = overlap / len(output_tokens)
    recall = overlap / len(target_tokens)
    return 2 * precision * recall / (precision + recall)


def _normalize(value: str, *, lowercase: bool, strip: bool) -> str:
    normalized = value.strip() if strip else value
    return normalized.lower() if lowercase else normalized


def _parse_float(value: str) -> float | None:
    try:
        parsed = float(value)
    except ValueError:
        return None
    return parsed if math.isfinite(parsed) else None


def _tokens(value: str) -> list[str]:
    return _normalize(value, lowercase=True, strip=True).split()


__all__ = ["exact_match", "f1", "multi_tolerance", "normalized_match"]
