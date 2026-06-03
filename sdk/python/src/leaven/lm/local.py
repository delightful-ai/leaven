"""`lv.lm.local(...)` — local LM provider config (llama.cpp, mlx, etc.)."""

from typing import Literal

from .config import LmCacheMode, LmConfig


class LocalLm(LmConfig):
    """Local LM config (llama.cpp / mlx / etc.)."""

    provider: Literal["local"] = "local"
    endpoint: str = "http://localhost:8080"
    """Local OpenAI-compatible HTTP endpoint."""


def local(
    *,
    model: str,
    endpoint: str = "http://localhost:8080",
    role: str | None = None,
    cache: LmCacheMode = "read_write",
    timeout_s: float | None = None,
    max_retries: int = 2,
) -> LocalLm:
    """Local LM provider config builder."""
    return LocalLm(
        model=model,
        endpoint=endpoint,
        role=role,
        cache=cache,
        timeout_s=timeout_s,
        max_retries=max_retries,
    )


__all__ = ["LocalLm", "local"]
