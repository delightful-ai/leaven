"""Public unsupported-status facts for run summaries."""

from typing import Literal

from pydantic import BaseModel, ConfigDict

UnsupportedSurface = Literal["run.cost", "run.usage", "run.inspection"]
UnsupportedReason = Literal[
    "provider_cost_not_reported",
    "provider_usage_not_reported",
    "blob_readback_not_implemented",
]


class UnsupportedRunFact(BaseModel):
    """A public fact explaining why a run summary field is intentionally unknown."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    surface: UnsupportedSurface
    dependency: str
    reason: UnsupportedReason
    detail: str


def has_unsupported_surface(
    facts: tuple[UnsupportedRunFact, ...],
    surface: UnsupportedSurface,
) -> bool:
    """Return whether any fact marks a public result surface unsupported."""
    return any(fact.surface == surface for fact in facts)


__all__ = [
    "UnsupportedReason",
    "UnsupportedRunFact",
    "UnsupportedSurface",
    "has_unsupported_surface",
]
