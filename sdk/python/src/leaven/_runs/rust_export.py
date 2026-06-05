"""Private bridge to Rust-owned run inspection exports."""

import json
import os
import subprocess
from pathlib import Path

from ..run_inspection import (
    BlobReadbackSummary,
    EvidenceReadbackRef,
    RustBlobReadback,
    RustEvidenceReadback,
    RustRunReadback,
)


def load_rust_run_readback(
    path: str | Path,
    *,
    leaven_bin: Path | None = None,
    timeout_s: int = 60,
) -> RustRunReadback | None:
    """Return Rust-owned checkpoint/graph readback when `path` has a checkpoint."""
    run_dir = _run_dir(path)
    if not (run_dir / "checkpoints" / "LATEST").is_file():
        return None
    binary = leaven_bin or _resolve_leaven_binary()
    process = subprocess.run(
        [
            str(binary),
            "run",
            "inspect",
            "--run-dir",
            str(run_dir),
        ],
        text=True,
        capture_output=True,
        timeout=timeout_s,
        check=False,
    )
    if process.returncode != 0:
        raise RuntimeError(
            "leaven run inspect failed\n"
            f"status: {process.returncode}\nstdout:\n{process.stdout}\nstderr:\n{process.stderr}"
        )
    return RustRunReadback.model_validate(json.loads(process.stdout))


def load_rust_blob_readback(
    path: str | Path,
    blob: BlobReadbackSummary,
    *,
    leaven_bin: Path | None = None,
    timeout_s: int = 60,
) -> RustBlobReadback:
    """Return bytes for one Rust-owned blob reference from a local run store."""
    run_dir = _run_dir(path)
    binary = leaven_bin or _resolve_leaven_binary()
    process = subprocess.run(
        [
            str(binary),
            "run",
            "blob",
            "--run-dir",
            str(run_dir),
            "--store",
            blob.store,
            "--key",
            blob.key,
        ],
        text=True,
        capture_output=True,
        timeout=timeout_s,
        check=False,
    )
    if process.returncode != 0:
        raise RuntimeError(
            "leaven run blob failed\n"
            f"status: {process.returncode}\nstdout:\n{process.stdout}\nstderr:\n{process.stderr}"
        )
    return RustBlobReadback.model_validate(json.loads(process.stdout))


def load_rust_evidence_readback(
    path: str | Path,
    evidence: EvidenceReadbackRef,
    *,
    leaven_bin: Path | None = None,
    timeout_s: int = 60,
) -> RustEvidenceReadback:
    """Return bytes for one Rust-owned evidence reference from a local run store."""
    run_dir = _run_dir(path)
    binary = leaven_bin or _resolve_leaven_binary()
    process = subprocess.run(
        [
            str(binary),
            "run",
            "evidence",
            "--run-dir",
            str(run_dir),
            "--store",
            evidence.store,
            "--key",
            evidence.key,
        ],
        text=True,
        capture_output=True,
        timeout=timeout_s,
        check=False,
    )
    if process.returncode != 0:
        raise RuntimeError(
            "leaven run evidence failed\n"
            f"status: {process.returncode}\nstdout:\n{process.stdout}\nstderr:\n{process.stderr}"
        )
    return RustEvidenceReadback.model_validate(json.loads(process.stdout))


def _run_dir(path: str | Path) -> Path:
    candidate = Path(path)
    if candidate.is_file():
        return candidate.parent
    return candidate


def _resolve_leaven_binary() -> Path:
    override = os.environ.get("LEAVEN_BIN")
    if override:
        return Path(override)
    repo_root = _resolve_repo_root()
    for profile in ("debug", "release"):
        candidate = repo_root / "target" / profile / "leaven"
        if candidate.is_file():
            return candidate
    raise FileNotFoundError("could not find leaven binary; set LEAVEN_BIN")


def resolve_leaven_binary() -> Path:
    """Return the configured Leaven CLI binary for private Rust run commands."""
    return _resolve_leaven_binary()


def _resolve_repo_root() -> Path:
    override = os.environ.get("LEAVEN_REPO_ROOT")
    if override:
        return Path(override)
    for start in (Path.cwd().resolve(), Path(__file__).resolve()):
        for parent in (start, *start.parents):
            if (parent / "Cargo.toml").is_file() and (
                parent / "crates" / "leaven" / "tests" / "topology_contract.rs"
            ).is_file():
                return parent
    raise FileNotFoundError("could not resolve Leaven repo root; set LEAVEN_REPO_ROOT")


__all__ = [
    "load_rust_blob_readback",
    "load_rust_evidence_readback",
    "load_rust_run_readback",
    "resolve_leaven_binary",
]
