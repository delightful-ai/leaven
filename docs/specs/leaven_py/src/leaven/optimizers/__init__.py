"""`lv.optimizers.*` — the optimizer registry.

`gepa` is the ONLY behavior-bearing optimizer in V1. `mipro`, `textgrad`, and
`trace` are RESERVED names that raise `NotImplementedError`. New optimizers
require a new Rust crate; Python users configure existing ones.

Governing spec: `docs/specs/leaven_python.md` — Optimizers.
"""

from __future__ import annotations

from pydantic import BaseModel, ConfigDict

__all__ = ["Optimizer", "gepa", "mipro", "textgrad", "trace"]


class Optimizer(BaseModel):
    """Base marker for optimizer configs."""

    model_config = ConfigDict(frozen=True, extra="forbid", arbitrary_types_allowed=True)

    kind: str


from .gepa import gepa  # noqa: E402
from .mipro import mipro  # noqa: E402
from .textgrad import textgrad  # noqa: E402
from .trace import trace  # noqa: E402
