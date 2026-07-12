"""Outcome rendering tests for live Terminal-Bench runs."""

import leaven as lv

from codex_terminal_bench.output import print_optimization_outcome


def test_print_outcome_reports_non_improvement_without_raising(capsys) -> None:
    """Regression: a valid seed-best live run should report the cutoff miss, not crash."""
    result = _optimized(seed_score=0.0, best_score=0.0)

    print_optimization_outcome(result)

    out = capsys.readouterr().out
    assert "improved:          False" in out
    assert "cutoff not met" in out


def _optimized(*, seed_score: float, best_score: float) -> lv.Optimized:
    seed = lv.Candidate(
        id="seed",
        artifact=lv.AgentKitArtifact(system_prompt="seed", skills=[]),
        parent_id=None,
        summary_score=seed_score,
    )
    best = seed
    if best_score != seed_score:
        best = lv.Candidate(
            id="child",
            artifact=lv.AgentKitArtifact(system_prompt="child", skills=[]),
            parent_id="seed",
            summary_score=best_score,
        )
    frontier = [seed] if best.id == seed.id else [seed, best]
    return lv.Optimized(
        run_id="run_1",
        best=best,
        frontier=frontier,
        summary=lv.RunSummary(
            run_id="run_1",
            started_at="2026-07-12T00:00:00Z",
            completed_at="2026-07-12T00:00:01Z",
            iterations=1,
            candidates_evaluated=len(frontier),
            total_cost_usd=None,
            total_calls=1,
            total_lm_tokens=None,
            run_dir=None,
            replayability="boundary_managed",
        ),
    )
