"""GEPA reflective-dataset hook — `reflective_dataset=` build-once-pass-down.

The hook runs engine-side as GEPA policy and hands the reflector a finished
batch. Governing spec: `docs/specs/leaven_python.md` — Reflect / Optimizers.
"""

from __future__ import annotations

from collections.abc import Awaitable, Callable

from ..adapters.reflective import ReflectiveCase, ReflectiveContext

__all__ = ["ReflectiveDatasetHook"]


type ReflectiveDatasetHook = Callable[[ReflectiveContext], Awaitable[list[ReflectiveCase]]]
"""`case evidence -> ReflectiveCase records`, run engine-side as GEPA policy."""
