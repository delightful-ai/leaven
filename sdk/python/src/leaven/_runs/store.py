"""Private run-directory discovery for Rust-owned Python SDK run readback."""

from pathlib import Path


def list_run_dirs(root: str | Path = ".leaven/runs") -> list[str]:
    """List run directory names with Rust-owned checkpoint state."""
    root_path = Path(root)
    if not root_path.exists():
        return []
    return sorted(
        path.name
        for path in root_path.iterdir()
        if path.is_dir() and _is_run_dir(path)
    )


def _is_run_dir(path: Path) -> bool:
    return (path / "checkpoints" / "LATEST").is_file()


__all__ = ["list_run_dirs"]
