"""`lv.lm.*` — provider-neutral LM config builders.

`lv.lm.anthropic / openai / local / mock` each return a typed `LmConfig` the
engine instantiates. Multiple configs can be wired into one runtime.

Governing spec: `docs/specs/leaven_python.md` — Runtime / lm.
"""

from __future__ import annotations

from .config import LmConfig

__all__ = ["LmConfig", "anthropic", "local", "mock", "openai"]


def anthropic(*, model: str, **kwargs: object) -> LmConfig:
    """Anthropic provider config (`lv.lm.anthropic(model="claude-opus-4-7")`)."""
    raise NotImplementedError("see leaven_python.md — Runtime / lm")


def openai(*, model: str, **kwargs: object) -> LmConfig:
    """OpenAI provider config (`lv.lm.openai(model=...)`)."""
    raise NotImplementedError("see leaven_python.md — Runtime / lm")


def local(*, model: str | None = None, **kwargs: object) -> LmConfig:
    """Local provider config (`lv.lm.local(...)`)."""
    raise NotImplementedError("see leaven_python.md — Runtime / lm")


def mock(**kwargs: object) -> LmConfig:
    """Mock provider config for tests (`lv.lm.mock()`)."""
    raise NotImplementedError("see leaven_python.md — Runtime / lm")
