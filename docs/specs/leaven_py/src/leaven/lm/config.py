"""LM config — internal frozen dataclass.

Provider-neutral LM config (internal config, not a wire record).

Governing spec: `docs/specs/leaven_python.md` — Runtime / lm.
"""

from __future__ import annotations

from dataclasses import dataclass

__all__ = ["LmConfig"]


@dataclass(frozen=True, slots=True)
class LmConfig:
    """LM config produced by `lv.lm.*` builders."""

    provider: str
    model: str | None = None
