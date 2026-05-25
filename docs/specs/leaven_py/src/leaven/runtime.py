"""`lv.runtime(...)` — product-facing run runtime builder.

This is the stage-composition name for the same boundary previously exposed as
`lv.environment(...)`: workspace allocation, LM/agent/sandbox access, trust,
budget, and cache policy. `lv.environment` remains as the legacy spelling while
the surface moves toward `runtime`.
"""

from __future__ import annotations

from .environment import Cache, Environment, environment

Runtime = Environment
"""Runtime configuration used by `lv.evolve(...)`."""

runtime = environment
"""Runtime builder; accepts the same arguments as `lv.environment(...)`."""

__all__ = ["Cache", "Runtime", "runtime"]
