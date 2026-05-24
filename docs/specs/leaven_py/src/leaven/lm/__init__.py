"""LM provider configs — `lv.lm.anthropic(...)`, `lv.lm.openai(...)`, etc.

Each builder returns a typed `LmConfig` the engine instantiates. Multiple
configs can be wired into one environment (`lm=[anthropic(...), openai(...)]`)
or per-role (`lm={"grader": anthropic(...), "reflector": openai(...)}`).
"""

from __future__ import annotations

from .anthropic import AnthropicLm, anthropic
from .config import LmConfig
from .local import LocalLm, local
from .mock import MockLm, mock
from .openai import OpenAiLm, openai

__all__ = [
    "AnthropicLm",
    "LmConfig",
    "LocalLm",
    "MockLm",
    "OpenAiLm",
    "anthropic",
    "local",
    "mock",
    "openai",
]
