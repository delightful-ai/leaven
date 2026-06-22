"""Adapter namespaces reserved for wired external integrations.

Per spec: artifact and provider semantics that aren't core Leaven live under
`x.*` so the core import surface stays clean. Each adapter ships its own
typed payloads, schema fingerprints, and capability constraints.
"""

from . import harbor

__all__ = ["harbor"]
