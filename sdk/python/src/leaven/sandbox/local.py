"""`lv.sandbox.local()` — local-process sandbox (trusted_local_operator only)."""

from typing import Literal

from .config import SandboxConfig


class LocalSandbox(SandboxConfig):
    """Local-process sandbox; only valid under `trusted_local_operator` trust profile."""

    backend: Literal["local"] = "local"


def local() -> LocalSandbox:
    """Local-process sandbox config.

    Permits unsandboxed subprocess execution on the operator's machine. The
    engine refuses this backend under non-`trusted_local_operator` trust
    profiles.
    """
    return LocalSandbox()


__all__ = ["LocalSandbox", "local"]
