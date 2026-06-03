"""Budget composition — `lv.budget(usd=..., calls=...)` for runs and stages.

Budgets are enforced engine-side via the locked capability document; the
Python builder is a typed declaration that lowers into the wire shape.
"""

from pydantic import BaseModel, ConfigDict


class Budget(BaseModel):
    """A budget envelope; pass via `lv.runtime(budget=...)`."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    usd: float | None = None
    """Aggregate USD cap across all costful effects in the run."""

    calls: int | None = None
    """Aggregate call count cap (LM + agent + sandbox combined)."""

    lm_tokens: int | None = None
    """Aggregate token count cap across all LM calls."""

    wall_seconds: float | None = None
    """Wall-clock cap for the entire run."""

    concurrent_calls: int | None = None
    """Concurrency cap on simultaneous in-flight effect calls."""


def budget(
    *,
    usd: float | None = None,
    calls: int | None = None,
    lm_tokens: int | None = None,
    wall_seconds: float | None = None,
    concurrent_calls: int | None = None,
) -> Budget:
    """Build a budget envelope.

    All fields optional; unset means no cap on that dimension. At least one
    cap should be set in practice (the engine warns on fully-uncapped budgets).
    """
    return Budget(
        usd=usd,
        calls=calls,
        lm_tokens=lm_tokens,
        wall_seconds=wall_seconds,
        concurrent_calls=concurrent_calls,
    )


__all__ = ["Budget", "budget"]
