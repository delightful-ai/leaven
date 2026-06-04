"""Environment and service configuration for the live OpenAI LM proof."""

import os
from dataclasses import dataclass

EXPECTED_TEXT = "LEAVEN_LIVE_LM_OK"
LIVE_ENV = "LEAVEN_LIVE_OPENAI"
OPENAI_KEY_ENV_OVERRIDE = "LEAVEN_OPENAI_API_KEY_ENV"
OPENAI_MODEL_ENV = "LEAVEN_OPENAI_MODEL"


@dataclass(frozen=True)
class LiveOpenAiConfig:
    """Resolved operator-controlled settings for the live OpenAI proof."""

    enabled: bool
    model: str
    api_key_env: str

    @classmethod
    def from_env(cls) -> "LiveOpenAiConfig":
        """Read the live-proof settings from environment variables."""
        return cls(
            enabled=os.environ.get(LIVE_ENV) == "1",
            model=os.environ.get(OPENAI_MODEL_ENV, "gpt-4.1-mini"),
            api_key_env=os.environ.get(OPENAI_KEY_ENV_OVERRIDE, "OPENAI_API_KEY"),
        )


__all__ = [
    "EXPECTED_TEXT",
    "LIVE_ENV",
    "OPENAI_KEY_ENV_OVERRIDE",
    "OPENAI_MODEL_ENV",
    "LiveOpenAiConfig",
]
