"""`lv.lm.anthropic(...)` — Anthropic LM provider config."""

from typing import Literal

from .config import LmCacheMode, LmConfig


class AnthropicLm(LmConfig):
    """Anthropic-specific config."""

    provider: Literal["anthropic"] = "anthropic"
    api_key_env: str = "ANTHROPIC_API_KEY"
    """Env var the engine reads the bearer from. Never pass keys in code."""

    extended_thinking: bool = False
    """Enable extended thinking for supported models."""


def anthropic(
    *,
    model: str,
    role: str | None = None,
    api_key_env: str = "ANTHROPIC_API_KEY",
    extended_thinking: bool = False,
    cache: LmCacheMode = "read_write",
    timeout_s: float | None = None,
    max_retries: int = 2,
) -> AnthropicLm:
    """Anthropic LM provider config builder.

    Example:
        env = lv.runtime(lm=lv.lm.anthropic(model="claude-opus-4-7"))
    """
    return AnthropicLm(
        model=model,
        role=role,
        api_key_env=api_key_env,
        extended_thinking=extended_thinking,
        cache=cache,
        timeout_s=timeout_s,
        max_retries=max_retries,
    )


__all__ = ["AnthropicLm", "anthropic"]
