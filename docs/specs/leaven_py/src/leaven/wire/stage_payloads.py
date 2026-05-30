"""Wire records: stage payload types with cross-stage binding.

`StageRole`, `StageSourceRef`, `ReflectExample`, `ReflectRequest`,
`ReflectionResult`, `ProposeRequest`, `JudgeRequest`. Governing spec:
`docs/specs/leaven_python.md` — Stage payload typing. Schema owned by
`docs/specs/public-seam-v1/schemas/`.
"""

from __future__ import annotations

from collections.abc import Mapping, Sequence
from enum import StrEnum
from typing import Any

from pydantic import BaseModel, ConfigDict

__all__ = [
    "JudgeRequest",
    "ProposeRequest",
    "ReflectExample",
    "ReflectRequest",
    "ReflectionResult",
    "StageRole",
    "StageSourceRef",
]


class StageRole(StrEnum):
    """The five stage roles carried across the wire."""

    runner = "runner"
    scorer = "scorer"
    reflector = "reflector"
    proposer = "proposer"
    evaluator = "evaluator"


class StageSourceRef(BaseModel):
    """Cross-stage source reference binding a payload to its producing stage."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    stage: StageRole
    receipt_id: str


class ReflectExample(BaseModel):
    """One target-safe example in a reflect request."""

    model_config = ConfigDict(frozen=True, extra="forbid", arbitrary_types_allowed=True)

    input: Mapping[str, Any]
    output: Any
    feedback: str = ""
    source: StageSourceRef | None = None


class ReflectRequest(BaseModel):
    """The build-once-pass-down reflect request handed to a reflector."""

    model_config = ConfigDict(frozen=True, extra="forbid", arbitrary_types_allowed=True)

    examples: Sequence[ReflectExample]
    parent: object | None = None


class ReflectionResult(BaseModel):
    """The reflector's digested output, fed to propose (not the raw batch)."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    summary: str
    failure_modes: Sequence[str] = ()
    suggestions: Sequence[str] = ()
    constraints: Sequence[str] = ()


class ProposeRequest(BaseModel):
    """The propose request: parent + digested reflection + reflector receipt."""

    model_config = ConfigDict(frozen=True, extra="forbid", arbitrary_types_allowed=True)

    parent: object
    reflection: ReflectionResult
    reflector_receipt: StageSourceRef | None = None


class JudgeRequest(BaseModel):
    """A pairwise/listwise judge request."""

    model_config = ConfigDict(frozen=True, extra="forbid", arbitrary_types_allowed=True)

    items: Sequence[object]
    rubric: str | None = None
