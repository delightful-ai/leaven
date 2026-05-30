"""`lv.wire.*` — generated public-seam schema records.

These are the floorboard creatures: engine/wire nouns that exist but are NOT
product nouns. They are forbidden from the top-level `lv.__all__`; authors who
need them import explicitly from `lv.wire`.

All records are pydantic v2 frozen, `extra="forbid"`, snake_case fields. The
exact schema is owned by `docs/specs/public-seam-v1/schemas/`; these are the
minimal typed projection.

Governing spec: `docs/specs/leaven_python.md` — Public API discipline.
"""

from __future__ import annotations

from .assessment import AssessmentWrite, Replayability
from .evaluation_job import EvaluationItem, EvaluationJob, Granularity, Purpose
from .evidence import EvidenceEnvelope, EvidencePrivate, EvidencePublic
from .output_record import OutputRecord
from .proposal import ProposalBatch, ProposalEffect
from .receipts import CallReceipt, QueryReceipt, WriteReceipt
from .stage_payloads import (
    JudgeRequest,
    ProposeRequest,
    ReflectExample,
    ReflectionResult,
    ReflectRequest,
    StageRole,
    StageSourceRef,
)
from .visibility import Visibility

__all__ = [
    "AssessmentWrite",
    "CallReceipt",
    "EvaluationItem",
    "EvaluationJob",
    "EvidenceEnvelope",
    "EvidencePrivate",
    "EvidencePublic",
    "Granularity",
    "JudgeRequest",
    "OutputRecord",
    "ProposalBatch",
    "ProposalEffect",
    "ProposeRequest",
    "Purpose",
    "QueryReceipt",
    "ReflectExample",
    "ReflectRequest",
    "ReflectionResult",
    "Replayability",
    "StageRole",
    "StageSourceRef",
    "Visibility",
    "WriteReceipt",
]
