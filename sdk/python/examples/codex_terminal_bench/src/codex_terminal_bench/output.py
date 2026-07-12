"""Render the optimization outcome and report whether the live cutoff held.

The no-spend mechanics test owns the hard cutoff assertion. The live proof can
legitimately spend its budget without admitting a better child, so this module
prints the seed and evolved kit, the frontier scores, and the run/cost facts
without crashing by default.
"""

import os

import leaven as lv

ASSERT_IMPROVED_ENV = "LEAVEN_CODEX_TB_ASSERT_IMPROVED"
_CUTOFF_MESSAGE = (
    "cutoff not met: the evolved kit did not beat the seed. Expected a CHANGED "
    "kit, applied and re-evaluated onto the frontier, with a strictly higher "
    "score than the seed."
)


def print_optimization_outcome(
    result: lv.Optimized,
    *,
    assert_improved: bool | None = None,
) -> None:
    """Print the seed/best kits, frontier scores, and run facts.

    By default a non-improving live run is reported, not raised, because valid
    live runs can exhaust their budget without beating the seed. Set
    `assert_improved` or `LEAVEN_CODEX_TB_ASSERT_IMPROVED=1` for cutoff checks.
    """
    seed = next(c for c in result.frontier if c.parent_id is None)
    seed_score = seed.summary_score or 0.0
    best_score = result.best.summary_score or 0.0
    improved = result.best.id != seed.id and best_score > seed_score

    print(f"run id:            {result.run_id}")
    print(f"seed score:        {seed_score:.3f}")
    print(f"best score:        {best_score:.3f}")
    print(f"improved:          {improved}")
    print(f"iterations:        {result.summary.iterations}")
    print(f"metric calls used: {result.summary.total_calls}")
    print(f"lm tokens:         {result.summary.total_lm_tokens}")
    print(f"cost (usd):        {result.summary.total_cost_usd}")
    print(f"cost status:       {result.summary.cost_status}")
    print(f"run dir:           {result.summary.run_dir}")
    print(f"frontier size:     {len(result.frontier)}")

    seed_kit = _as_kit(seed.artifact)
    best_kit = _as_kit(result.best.artifact)
    print("\nseed AGENTS.md:")
    print(seed_kit.system_prompt)
    print("\noptimized AGENTS.md:")
    print(best_kit.system_prompt)
    if best_kit.skills:
        print("\noptimized skills:")
        for skill in best_kit.skills:
            print(f"- {skill.path} ({len(skill.content)} chars)")

    if not improved:
        print(f"\n{_CUTOFF_MESSAGE}")

    should_assert = assert_improved
    if should_assert is None:
        should_assert = os.environ.get(ASSERT_IMPROVED_ENV) == "1"
    if should_assert and not improved:
        raise AssertionError(_CUTOFF_MESSAGE)


def _as_kit(artifact: object) -> lv.AgentKitArtifact:
    if not isinstance(artifact, lv.AgentKitArtifact):
        raise TypeError(f"expected an AgentKitArtifact candidate; got {type(artifact).__name__}")
    return artifact


__all__ = ["ASSERT_IMPROVED_ENV", "print_optimization_outcome"]
