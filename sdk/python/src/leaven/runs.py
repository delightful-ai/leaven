"""`lv.runs.open(path)` — inspect a completed run from outside.

Same `Optimized[A]` type as `lv.optimize(...).run()` returns; the engine is
spawned read-only against the run directory. Useful for retrospective
analysis, ablation reports, sharing run state with teammates.
"""

from pathlib import Path

from ._runs import (
    list_run_dirs,
    load_rust_blob_readback,
    load_rust_evidence_readback,
    load_rust_run_readback,
    open_optimized,
    open_rust_optimized,
    optimized_from_rust_readback,
)
from .result import Optimized
from .run_inspection import RunInspection, inspect_optimized


def open(path: str | Path) -> Optimized[object]:
    """Open a completed run from its run directory.

    The artifact type is `object` because the run's artifact type is
    determined at write time; callers can narrow if they know the type. A
    future API revision may make this generic over a passed artifact decoder.
    """
    rust_result = open_rust_optimized(path)
    if rust_result is not None:
        return rust_result
    return open_optimized(path)


def list_local(root: str | Path = ".leaven/runs") -> list[str]:
    """List run directory names under the local leaven root."""
    return list_run_dirs(root)


def inspect(path: str | Path) -> RunInspection:
    """Open a completed run and return a flattened inspection summary."""
    rust_readback = load_rust_run_readback(path)
    rust_graph_blob = (
        load_rust_blob_readback(path, rust_readback.graph.blob)
        if rust_readback is not None
        else None
    )
    rust_evidence = (
        [
            load_rust_evidence_readback(path, assessment.evidence)
            for assessment in rust_readback.graph.assessments
        ]
        if rust_readback is not None
        else []
    )
    result = (
        optimized_from_rust_readback(rust_readback, run_dir=str(_run_dir(path)))
        if rust_readback is not None
        else open_optimized(path)
    )
    return inspect_optimized(
        result,
        rust_readback=rust_readback,
        rust_graph_blob=rust_graph_blob,
        rust_evidence=rust_evidence,
    )


def _run_dir(path: str | Path) -> Path:
    candidate = Path(path)
    if candidate.is_file():
        return candidate.parent
    return candidate


__all__ = ["RunInspection", "inspect", "list_local", "open"]
