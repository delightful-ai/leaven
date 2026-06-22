"""Reward helpers that score structured Harbor rollout evidence."""

from leaven.x.harbor._types import HarborTrialOutcome


def map_key(key: str, *, weight: float = 1.0, id: str | None = None) -> object:
    """Score a named Harbor verifier reward as a normal Leaven reward."""
    import leaven as lv  # noqa: PLC0415

    @lv.reward(weight=weight, id=id or f"leaven.x.harbor.rewards.{key}")
    async def score(output: str, case: lv.ScoringCaseView, cx: lv.RubricContext) -> lv.RewardValue:
        _ = (case, cx)
        outcome = HarborTrialOutcome.decode(output)
        if key not in outcome.rewards:
            return lv.RewardValue(value=0.0, feedback=f"missing Harbor reward `{key}`")
        value = outcome.rewards[key]
        return lv.RewardValue(value=value, feedback=f"Harbor reward `{key}`: {value}")

    return score


def ctrf_fraction(*, weight: float = 1.0, id: str | None = None) -> object:
    """Score CTRF passed/total partial credit as a normal Leaven reward."""
    import leaven as lv  # noqa: PLC0415

    @lv.reward(weight=weight, id=id or "leaven.x.harbor.rewards.ctrf_fraction")
    async def score(output: str, case: lv.ScoringCaseView, cx: lv.RubricContext) -> lv.RewardValue:
        _ = (case, cx)
        outcome = HarborTrialOutcome.decode(output)
        if outcome.ctrf is None or outcome.ctrf.total <= 0:
            return lv.RewardValue(value=0.0, feedback="no CTRF total available")
        feedback = f"CTRF {outcome.ctrf.passed}/{outcome.ctrf.total} tests passed"
        if outcome.ctrf.failed_names:
            feedback += f"; failing: {', '.join(outcome.ctrf.failed_names)}"
        return lv.RewardValue(value=outcome.ctrf_fraction, feedback=feedback)

    return score


def default_rewards() -> list[object]:
    """Default Harbor reward vector: task reward plus CTRF partial credit."""
    return [map_key("reward"), ctrf_fraction(weight=0.25)]


__all__ = ["ctrf_fraction", "default_rewards", "map_key"]
