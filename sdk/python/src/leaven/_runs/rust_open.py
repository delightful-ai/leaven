"""Build public run handles from Rust-owned checkpoint readback."""

from pathlib import Path

from ..assessment import Replayability
from ..result import Candidate, Optimized, RunSummary
from ..run_inspection import RustRunReadback
from ..run_status import UnsupportedRunFact
from .rust_export import load_rust_run_readback


def open_rust_optimized(path: str | Path) -> Optimized[object] | None:
    """Open a completed run from Rust-owned checkpoint state when present."""
    readback = load_rust_run_readback(path)
    if readback is None:
        return None
    return optimized_from_rust_readback(readback, run_dir=str(_run_dir(path)))


def optimized_from_rust_readback(
    readback: RustRunReadback,
    *,
    run_dir: str | None,
) -> Optimized[object]:
    """Project Rust-owned graph readback into the public Optimized handle."""
    frontier = [
        Candidate[object](
            id=candidate.id,
            artifact=candidate.artifact,
            parent_id=candidate.parent_id,
        )
        for candidate in readback.graph.candidates
    ]
    if not frontier:
        raise ValueError("Rust run readback has no candidates")
    best_id = readback.graph.best_candidate_id
    if best_id is None:
        raise ValueError("Rust run readback has no completed-run best candidate")
    best = _candidate_by_id(frontier, best_id)
    return Optimized[object](
        run_id=readback.run_id,
        best=best,
        frontier=frontier,
        summary=RunSummary(
            run_id=readback.run_id,
            started_at="",
            completed_at=None,
            iterations=readback.graph.event_count,
            candidates_evaluated=readback.graph.assessment_count,
            total_cost_usd=None,
            cost_status="unsupported_dependency",
            total_calls=readback.graph.event_count,
            total_lm_tokens=None,
            usage_status="unsupported_dependency",
            unsupported=(
                UnsupportedRunFact(
                    surface="run.cost",
                    dependency="Rust checkpoint inspection",
                    reason="provider_cost_not_reported",
                    detail="Rust run-open readback does not yet export cost totals.",
                ),
                UnsupportedRunFact(
                    surface="run.usage",
                    dependency="Rust checkpoint inspection",
                    reason="provider_usage_not_reported",
                    detail="Rust run-open readback does not yet export usage totals.",
                ),
                UnsupportedRunFact(
                    surface="run.inspection",
                    dependency="Rust checkpoint inspection",
                    reason="blob_readback_not_implemented",
                    detail="Rust run-open readback does not yet export assessment rows.",
                ),
            ),
            run_dir=run_dir,
            replayability=_replayability(),
        ),
    )


def _candidate_by_id(candidates: list[Candidate[object]], candidate_id: str) -> Candidate[object]:
    for candidate in candidates:
        if candidate.id == candidate_id:
            return candidate
    raise KeyError(f"Rust run readback best candidate {candidate_id!r} is missing")


def _replayability() -> Replayability:
    return "boundary_managed"


def _run_dir(path: str | Path) -> Path:
    candidate = Path(path)
    if candidate.is_file():
        return candidate.parent
    return candidate


__all__ = ["open_rust_optimized", "optimized_from_rust_readback"]
