"""Private run-directory store for Python SDK inspection results."""

import json
from pathlib import Path
from typing import Any

from ..result import Optimized
from .codec import decode_optimized, encode_optimized

RUN_RESULT_FILE = "optimized.json"


def persist_optimized(
    result: Optimized[Any],
    *,
    root: str | Path = ".leaven/runs",
) -> Optimized[Any]:
    """Persist an optimized result and return the copy carrying its run directory."""
    run_dir = Path(root) / _run_dir_name(result.run_id)
    summary = result.summary.model_copy(update={"run_dir": str(run_dir)})
    persisted = result.model_copy(update={"summary": summary})
    run_dir.mkdir(parents=True, exist_ok=True)
    target = run_dir / RUN_RESULT_FILE
    tmp = run_dir / f".{RUN_RESULT_FILE}.tmp"
    tmp.write_text(
        json.dumps(encode_optimized(persisted), sort_keys=True, indent=2) + "\n",
        encoding="utf-8",
    )
    tmp.replace(target)
    return persisted


def open_optimized(path: str | Path) -> Optimized[Any]:
    """Open a persisted optimized result from a run directory or result file."""
    result_path = _result_path(Path(path))
    return decode_optimized(json.loads(result_path.read_text(encoding="utf-8")))


def list_run_dirs(root: str | Path = ".leaven/runs") -> list[str]:
    """List persisted run directory names under the local root."""
    root_path = Path(root)
    if not root_path.exists():
        return []
    return sorted(
        path.name
        for path in root_path.iterdir()
        if path.is_dir() and (path / RUN_RESULT_FILE).is_file()
    )


def _result_path(path: Path) -> Path:
    if path.is_dir():
        return path / RUN_RESULT_FILE
    return path


def _run_dir_name(run_id: str) -> str:
    cleaned = "".join(ch if ch.isalnum() or ch in "._-" else "_" for ch in run_id)
    return cleaned or "leaven_run"


__all__ = ["RUN_RESULT_FILE", "list_run_dirs", "open_optimized", "persist_optimized"]
