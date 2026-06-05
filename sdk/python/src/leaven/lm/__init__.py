"""LM provider configs — `lv.lm.anthropic(...)`, `lv.lm.openai(...)`, etc.

Each builder returns a typed `LmConfig` the engine instantiates. Multiple
configs can be wired into one runtime (`lm=[anthropic(...), openai(...)]`)
or per-role (`lm={"grader": anthropic(...), "reflector": openai(...)}`).
"""

from ..builders.lm import LmMessage, LmMessageRole, LmResponse, LmTool
from .anthropic import AnthropicLm, anthropic
from .config import LmConfig
from .local import LocalLm, local
from .mock import MockLm, mock
from .openai import OpenAiLm, openai

__all__ = [
    "AnthropicLm",
    "LmConfig",
    "LmMessage",
    "LmMessageRole",
    "LmResponse",
    "LmTool",
    "LocalLm",
    "MockLm",
    "OpenAiLm",
    "anthropic",
    "local",
    "mock",
    "openai",
]
