"""Adapter namespaces — `lv.x.dspy.*`, future `lv.x.skill_bank.*`, etc.

Per spec: artifact and provider semantics that aren't core Leaven live under
`x.*` so the core import surface stays clean. Each adapter ships its own
typed payloads, schema fingerprints, and capability constraints.
"""

from __future__ import annotations

from . import dspy

__all__ = ["dspy"]
