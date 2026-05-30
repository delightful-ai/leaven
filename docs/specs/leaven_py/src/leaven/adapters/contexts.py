"""Typed context — `EvalContext`.

There is ONE product context: `lv.Context` (owned by `leaven.context`). Per-role
capability differences are token-enforced at runtime (`CapabilityError`), NOT
modeled as a distinct Python type per role — so there is no `RunContext` or
`StageContext`.

`EvalContext` is the ONLY context subtype: it extends `Context` with the
advanced batched-effect surface (`submit` / `case.load(include=)` /
`materialize_candidate`) used by the `@lv.evaluator` advanced authoring path. It
adds fields on the eval boundary, not optionally across boundaries (a runner
simply never receives an `EvalContext`).

Governing spec: `docs/specs/leaven_python.md` — Public API discipline /
constraints (single Context + EvalContext).
"""

from __future__ import annotations

from typing import TYPE_CHECKING, Protocol, runtime_checkable

from ..context import BatchContext, Context

if TYPE_CHECKING:
    from ..wire.assessment import AssessmentWrite

__all__ = ["EvalContext"]


@runtime_checkable
class EvalContext(Context, Protocol):
    """Advanced evaluator `cx`; extends `Context` with the batched-effect submit
    surface. `case.load(include=...)` and `materialize_candidate` come from the
    inherited `Context` handles."""

    def batch(self) -> BatchContext: ...
    def submit(self, write: AssessmentWrite) -> None: ...
