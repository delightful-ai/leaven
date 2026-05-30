"""`lv.x.*` — external-ecosystem adapters.

Typed integration with an external ecosystem, lowered into core Leaven types,
lifted back for the user. `dspy` is the proof case; `verifiers` / `harbor` are
reserved adapter namespaces.

Governing spec: `docs/specs/leaven_python.md` — DSPy / adapter namespaces.
"""

from __future__ import annotations

from . import dspy, harbor, verifiers

__all__ = ["dspy", "harbor", "verifiers"]
