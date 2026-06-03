"""Cost and usage status projection helpers for public run summaries."""

from __future__ import annotations

from dataclasses import dataclass
from typing import Literal

from .unsupported import UnsupportedRunFact, has_unsupported_surface

RunCostStatus = Literal["known", "unsupported_dependency"]
RunUsageStatus = Literal["known", "unsupported_dependency"]


@dataclass(frozen=True)
class RunCostProjection:
    """Projected public cost/usage totals plus their support status."""

    total_cost_usd: float | None
    total_lm_tokens: int | None
    cost_status: RunCostStatus
    usage_status: RunUsageStatus


def project_cost_usage(
    *,
    default_cost_usd: float,
    default_lm_tokens: int,
    unsupported: tuple[UnsupportedRunFact, ...],
) -> RunCostProjection:
    """Hide fabricated totals when a declared dependency cannot report them."""
    cost_status: RunCostStatus = (
        "unsupported_dependency" if has_unsupported_surface(unsupported, "run.cost") else "known"
    )
    usage_status: RunUsageStatus = (
        "unsupported_dependency" if has_unsupported_surface(unsupported, "run.usage") else "known"
    )
    return RunCostProjection(
        total_cost_usd=None if cost_status == "unsupported_dependency" else default_cost_usd,
        total_lm_tokens=None if usage_status == "unsupported_dependency" else default_lm_tokens,
        cost_status=cost_status,
        usage_status=usage_status,
    )


__all__ = [
    "RunCostProjection",
    "RunCostStatus",
    "RunUsageStatus",
    "project_cost_usage",
]
