"""Case — one immutable case in the task world.

`split` is a free user-defined label string (NOT a fixed train/val/test enum);
one label per case. `files` values are `lv.assets.path(...)` refs; `setup` is a
`lv.setup.bash(...)` step.

`target` is OPTIONAL — not every task is labeled (the seam's `Case<I, T=NoTarget>`).
For the labeled/supervised path, `case.expect("answer")` returns the target value
or raises a clear error, so a scorer never needs a bare `case.target["answer"]`
(which is `None`-unsafe) or an `assert`. For the unlabeled path, simply omit
`target=`; a target-free task scores from the rollout output/trajectory alone.

Cases project differently per consuming stage (a rollout never sees
`case.target`); those projections are engine-internal. Public users always
write `lv.Case`.

Governing spec: `docs/specs/leaven_python.md` — Task and Case.
"""

from __future__ import annotations

from collections.abc import Mapping
from typing import Any

from pydantic import BaseModel, ConfigDict

from .assets import AssetRef
from .setup import SetupStep

__all__ = ["Case"]


class Case(BaseModel):
    """One immutable case. `split` is a free user label, not a fixed enum.
    `target` is optional (not every task is labeled)."""

    model_config = ConfigDict(frozen=True, extra="forbid", arbitrary_types_allowed=True)

    id: str
    input: Mapping[str, Any]
    target: Mapping[str, Any] | None = None
    files: Mapping[str, AssetRef] | None = None
    setup: SetupStep | None = None
    split: str | None = None
    metadata: Mapping[str, Any] | None = None

    def expect(self, key: str) -> Any:
        """Return `target[key]` for the supervised path, or raise a clear error.

        Avoids the `None`-unsafe `case.target[key]` and the bare `assert` in a
        scorer: an unlabeled case (or a missing key) raises `KeyError` with a
        message naming the case, not an opaque `TypeError`.
        """
        raise NotImplementedError("see leaven_python.md — Task and Case (Case.expect)")
