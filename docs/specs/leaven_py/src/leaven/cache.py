"""Cache config — internal frozen dataclass (NOT a top-level product noun).

`runtime(cache=...)` accepts a `CacheConfig`. `lv.cache` is intentionally NOT
in the top-level allow-list; the cache surface is reached only via
`runtime(cache=...)`.

Governing spec: `docs/specs/leaven_python.md` — Runtime / cache.
"""

from __future__ import annotations

from dataclasses import dataclass

__all__ = ["CacheConfig", "sqlite_default"]


@dataclass(frozen=True, slots=True)
class CacheConfig:
    """Cache config passed via `runtime(cache=...)`."""

    kind: str


def sqlite_default() -> CacheConfig:
    """The one-knob SQLite-backed default cache (spec line 831)."""
    raise NotImplementedError("see leaven_python.md — Runtime / cache")
