"""LM config base — what every provider config produces."""

from typing import Literal

from pydantic import BaseModel, ConfigDict

LmCacheMode = Literal["off", "read_only", "read_write"]


class LmConfig(BaseModel):
    """Common LM config fields. Provider-specific subclasses add fields."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    provider: str
    """Provider name (e.g. 'anthropic', 'openai', 'local', 'mock')."""

    model: str
    """Provider-specific model id."""

    role: str | None = None
    """Optional role binding (e.g. 'reflector', 'grader')."""

    cache: LmCacheMode = "read_write"
    """Engine cache mode for this provider."""

    timeout_s: float | None = None
    """Per-call timeout."""

    max_retries: int = 2


__all__ = ["LmCacheMode", "LmConfig"]
