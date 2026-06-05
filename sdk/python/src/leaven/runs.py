"""`lv.runs.open(path)` — inspect a completed Rust-checkpointed run."""

from pathlib import Path

from ._runs import (
    list_run_dirs,
    load_rust_blob_readback,
    load_rust_evidence_readback,
    load_rust_run_readback,
    open_rust_optimized,
    optimized_from_rust_readback,
    rust_assessment_rows,
    rust_evidence_summaries,
)
from .artifacts.prompt import PromptArtifact
from .result import Optimized
from .run_inspection import RunInspection, inspect_optimized


def open(path: str | Path) -> Optimized[PromptArtifact]:
    """Open a completed run from its run directory.

    The current SDK checkpoint reader supports prompt-artifact runs only. Other
    artifact families must add typed Rust readback before this surface accepts
    them.
    """
    rust_result = open_rust_optimized(path)
    if rust_result is None:
        raise FileNotFoundError(f"no Rust checkpoint readback found at {Path(path)}")
    return rust_result


def list_local(root: str | Path = ".leaven/runs") -> list[str]:
    """List run directory names under the local leaven root."""
    return list_run_dirs(root)


def inspect(path: str | Path) -> RunInspection:
    """Open a completed run and return a flattened inspection summary."""
    rust_readback = load_rust_run_readback(path)
    if rust_readback is None:
        raise FileNotFoundError(f"no Rust checkpoint readback found at {Path(path)}")
    rust_graph_blob = load_rust_blob_readback(path, rust_readback.graph.blob)
    rust_artifact_blobs = [
        load_rust_blob_readback(path, blob) for blob in rust_readback.checkpoint.artifact_refs
    ]
    rust_stage_journal_blobs = [
        load_rust_blob_readback(path, blob) for blob in rust_readback.checkpoint.stage_journal_refs
    ]
    rust_workspace_journal_blobs = [
        load_rust_blob_readback(path, blob)
        for blob in rust_readback.checkpoint.workspace_journal_refs
    ]
    rust_evidence = [
        load_rust_evidence_readback(path, assessment.evidence)
        for assessment in rust_readback.graph.assessments
    ]
    evidence_summaries = rust_evidence_summaries(rust_readback, rust_evidence)
    assessment_rows = rust_assessment_rows(rust_readback, rust_evidence)
    result = optimized_from_rust_readback(
        rust_readback,
        run_dir=str(_run_dir(path)),
        assessment_rows=assessment_rows,
    )
    return inspect_optimized(
        result,
        rust_readback=rust_readback,
        rust_graph_blob=rust_graph_blob,
        rust_artifact_blobs=rust_artifact_blobs,
        rust_stage_journal_blobs=rust_stage_journal_blobs,
        rust_workspace_journal_blobs=rust_workspace_journal_blobs,
        rust_evidence=rust_evidence,
        evidence_summaries=evidence_summaries,
    )


def _run_dir(path: str | Path) -> Path:
    candidate = Path(path)
    if candidate.is_file():
        return candidate.parent
    return candidate


__all__ = ["RunInspection", "inspect", "list_local", "open"]
