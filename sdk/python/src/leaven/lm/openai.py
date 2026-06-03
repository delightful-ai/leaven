"""`lv.lm.openai(...)` — OpenAI LM provider config."""

from __future__ import annotations

from typing import Literal

from .config import LmCacheMode, LmConfig


class OpenAiLm(LmConfig):
    """OpenAI-specific config."""

    provider: Literal["openai"] = "openai"
    api_key_env: str = "OPENAI_API_KEY"
    base_url: str | None = None
    """Override for OpenAI-compatible endpoints."""
    reasoning_effort: str | None = None
    """`'low'`, `'medium'`, `'high'` for reasoning-capable models."""


def openai(
    *,
    model: str,
    role: str | None = None,
    api_key_env: str = "OPENAI_API_KEY",
    base_url: str | None = None,
    reasoning_effort: str | None = None,
    cache: LmCacheMode = "read_write",
    timeout_s: float | None = None,
    max_retries: int = 2,
) -> OpenAiLm:
    """OpenAI LM provider config builder.

    Pass `base_url` for any OpenAI-API-compatible endpoint (vLLM, OpenRouter,
    Together, etc.).
    """
    return OpenAiLm(
        model=model,
        role=role,
        api_key_env=api_key_env,
        base_url=base_url,
        reasoning_effort=reasoning_effort,
        cache=cache,
        timeout_s=timeout_s,
        max_retries=max_retries,
    )


__all__ = ["OpenAiLm", "openai"]
