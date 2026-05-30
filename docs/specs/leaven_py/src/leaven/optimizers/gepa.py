"""GEPA — the only behavior-bearing optimizer in V1.

`score=` is the primary-comparison scorer, passed as the scorer OBJECT (typed,
rename-safe), a name string (convenience), or a `lv.gepa.compare.*`
`CompareConfig`. `train=`/`validation=` accept the `lv.gepa.*` policy objects or
a split-name string. `reflective_dataset=` is the build-once-pass-down hook.

Governing spec: `docs/specs/leaven_python.md` — Optimizers.
"""

from __future__ import annotations

from typing import TYPE_CHECKING

from . import Optimizer

if TYPE_CHECKING:
    from ..gepa.compare import CompareConfig
    from ..gepa.component import ComponentPolicy
    from ..gepa.frontier import FrontierPolicy
    from ..gepa.gate import GatePolicy
    from ..gepa.reflective_dataset import ReflectiveDatasetHook
    from ..gepa.sampling import SamplingPolicy
    from ..gepa.validation import ValidationPolicy
    from ..score import Scorer

__all__ = ["gepa"]


def gepa(
    *,
    score: Scorer | str | CompareConfig,
    train: SamplingPolicy | str | None = None,
    validation: ValidationPolicy | str | None = None,
    population_size: int = 8,
    frontier: FrontierPolicy | None = None,
    reflective_dataset: ReflectiveDatasetHook | None = None,
    gate: GatePolicy | None = None,
    component: ComponentPolicy | None = None,
    **kwargs: object,
) -> Optimizer:
    """Configure the GEPA optimizer. Spec lines 1128-1166."""
    raise NotImplementedError("see leaven_python.md — Optimizers / gepa")
