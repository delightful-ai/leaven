"""Binary and repository discovery for the private public-seam client."""

from __future__ import annotations

import os
import shutil
from pathlib import Path

from .errors import SeamClientError


def resolve_repo_root() -> Path:
    """Locate the Leaven Cargo workspace root."""
    override = os.environ.get("LEAVEN_REPO_ROOT")
    if override:
        root = Path(override)
        if not (root / "Cargo.toml").is_file():
            raise SeamClientError(f"LEAVEN_REPO_ROOT={override!r} has no Cargo.toml")
        return root

    marker = Path("crates/leaven/tests/topology_contract.rs")
    for parent in Path(__file__).resolve().parents:
        if (parent / "Cargo.toml").is_file() and (parent / marker).is_file():
            return parent
    raise SeamClientError("could not locate Leaven repo root")


def resolve_leaven_binary() -> Path:
    """Locate the `leaven` CLI binary used to spawn the public seam server."""
    override = os.environ.get("LEAVEN_BIN")
    if override:
        return _existing_file(override, "LEAVEN_BIN")
    root = resolve_repo_root()
    for profile in ("debug", "release"):
        candidate = root / "target" / profile / "leaven"
        if candidate.is_file():
            return candidate
    raise SeamClientError("build the CLI first with `cargo build -p leaven-cli`, or set LEAVEN_BIN")


def resolve_codex_binary() -> str:
    """Locate the Codex CLI binary used by `CodexCliRuntimeConfig`."""
    override = os.environ.get("LEAVEN_CODEX_BIN")
    if override:
        return str(_existing_file(override, "LEAVEN_CODEX_BIN"))
    found = shutil.which("codex")
    if found:
        return found
    raise SeamClientError("could not find `codex`; set LEAVEN_CODEX_BIN")


def _existing_file(path: str, env_name: str) -> Path:
    candidate = Path(path)
    if not candidate.is_file():
        raise SeamClientError(f"{env_name}={path!r} is not a file")
    return candidate


__all__ = ["resolve_codex_binary", "resolve_leaven_binary", "resolve_repo_root"]
