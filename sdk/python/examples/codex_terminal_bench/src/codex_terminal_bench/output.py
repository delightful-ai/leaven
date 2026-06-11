"""Render the optimization outcome and assert the locked cutoff held.

The cutoff: a CHANGED kit authored by live Codex reflection from real trial
traces, applied through the run graph and re-evaluated onto the frontier beating
the seed. This module prints the seed and evolved kit, the frontier scores, and
the run/cost facts, and asserts the cutoff so a non-improving run fails loudly.
"""

import leaven as lv


def print_optimization_outcome(result: lv.Optimized) -> None:
    """Print the seed/best kits, frontier scores, and run facts; assert the cutoff."""
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

    assert improved, (
        "cutoff not met: the evolved kit did not beat the seed. Expected a CHANGED "
        "kit, applied and re-evaluated onto the frontier, with a strictly higher "
        "score than the seed."
    )


def _as_kit(artifact: object) -> lv.AgentKitArtifact:
    if not isinstance(artifact, lv.AgentKitArtifact):
        raise TypeError(f"expected an AgentKitArtifact candidate; got {type(artifact).__name__}")
    return artifact


__all__ = ["print_optimization_outcome"]
