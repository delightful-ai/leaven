"""Tests for `leaven.run_status.cost` projection."""

from leaven.run_status import UnsupportedRunFact, project_cost_usage


def test_run_status_projection_hides_unsupported_dependency_totals() -> None:
    """Example: unsupported provider facts prevent fabricated zero-cost summaries."""

    unsupported = (
        UnsupportedRunFact(
            surface="run.cost",
            dependency="codex_cli",
            reason="provider_cost_not_reported",
            detail="Codex CLI did not report cost.",
        ),
        UnsupportedRunFact(
            surface="run.usage",
            dependency="codex_cli",
            reason="provider_usage_not_reported",
            detail="Codex CLI did not report tokens.",
        ),
    )

    projection = project_cost_usage(
        default_cost_usd=0.0,
        default_lm_tokens=0,
        unsupported=unsupported,
    )

    assert projection.cost_status == "unsupported_dependency"
    assert projection.usage_status == "unsupported_dependency"
    assert projection.total_cost_usd is None
    assert projection.total_lm_tokens is None


__all__ = []
