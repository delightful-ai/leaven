"""`lv.adapters.*` — advanced authoring ring.

Evaluator support, `RegisteredStage`, the typed `EvalContext`, the reflective
types, and the handle/case annotation nouns. These are NOT product nouns;
ordinary runner/scorer code never imports them.

There is ONE product context, `lv.Context`; only `EvalContext` extends it here.
There is no `RunContext` / `StageContext` — per-role capability differences are
runtime-token-enforced, not Python-type-modeled.

Governing spec: `docs/specs/leaven_python.md` — Public API discipline.
"""

from __future__ import annotations

from .contexts import EvalContext
from .evaluator import Evaluator, EvaluatorFn
from .handles import (
    CandidateHandle,
    RunCase,
    ScoreCase,
    WorkspaceHandle,
    WorkspaceLifetime,
    WorkspaceSurface,
)
from .reflective import (
    Attachment,
    ReflectiveBatch,
    ReflectiveCase,
    ReflectiveContext,
    ReflectiveRun,
    TraceRef,
)
from .registered_stage import RegisteredStage

__all__ = [
    "Attachment",
    "CandidateHandle",
    "EvalContext",
    "Evaluator",
    "EvaluatorFn",
    "ReflectiveBatch",
    "ReflectiveCase",
    "ReflectiveContext",
    "ReflectiveRun",
    "RegisteredStage",
    "RunCase",
    "ScoreCase",
    "TraceRef",
    "WorkspaceHandle",
    "WorkspaceLifetime",
    "WorkspaceSurface",
]
