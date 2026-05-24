"""Built-in scoring helpers — common math users would otherwise rewrite.

Use directly (`lv.scoring.exact_match(...)`) or compose into your own scorer.
Not a complete catalog; user scorers can do anything via `@lv.scorer`.
"""

from __future__ import annotations

from collections.abc import Sequence


def exact_match(output: str, target: str) -> float:
    """1.0 iff `output == target` after default normalization; else 0.0."""
    raise NotImplementedError("scaffold; see docs/specs/leaven_python.md")


def normalized_match(output: str, target: str, *, lowercase: bool = True, strip: bool = True) -> float:
    """Exact match after optional lowercase/strip normalization."""
    raise NotImplementedError("scaffold; see docs/specs/leaven_python.md")


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
    raise NotImplementedError("scaffold; see docs/specs/leaven_python.md")


def f1(output: str, target: str) -> float:
    """Token-level F1 between output and target."""
    raise NotImplementedError("scaffold; see docs/specs/leaven_python.md")


__all__ = ["exact_match", "f1", "multi_tolerance", "normalized_match"]
