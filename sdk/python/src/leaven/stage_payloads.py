"""Stage payload types — `ReflectRequest`, `ProposeRequest`, `JudgeRequest`, etc.

What `@lv.reflector` / `@lv.proposer` / `@lv.judge` stages receive. Each is a
typed wire envelope produced by the engine and parsed on the Python side
before the user function runs.

These are the codegen targets for the eventual `leaven-types` package; the
hand-shaped versions here exist so the scaffold is importable and the
decorator signatures resolve.
"""

from typing import Any, Literal

from pydantic import BaseModel, ConfigDict, Field

from ._receipts import CallReceipt, QueryReceipt


class StageSourceRef(BaseModel):
    """Provenance ref for stage payload items (case ref, candidate ref, etc.)."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    kind: str
    id: str
    data_classes: list[str] = Field(default_factory=list)


class ReflectExample(BaseModel):
    """One reflection example fed to the reflector."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    case_id: str
    candidate_id: str
    feedback: str | None = None
    score: float | None = None
    source_refs: list[StageSourceRef] = Field(default_factory=list)


class ReflectRequest(BaseModel):
    """Payload for `@lv.reflector` stages."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    parent_candidate_id: str | None = None
    examples: list[ReflectExample]
    minibatch_size: int | None = None
    source_refs: list[StageSourceRef] = Field(default_factory=list)
    read_receipts: list[QueryReceipt] = Field(default_factory=list)


class ReflectionResult(BaseModel):
    """What `@lv.reflector` stages return."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    diagnosis: str
    """Structured diagnostic text the proposer will consume."""
    diagnosis_source_refs: list[StageSourceRef] = Field(default_factory=list)
    """Required: refs back to the examples the diagnosis depends on."""
    metadata: dict[str, Any] = Field(default_factory=dict)


class ProposeRequest(BaseModel):
    """Payload for `@lv.proposer` stages."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    parent_candidate_id: str
    reflection: ReflectionResult
    reflection_receipt: CallReceipt
    """Receipt binding the reflection — must be cited in the proposal submission."""
    allowed_change_schemas: list[str] = Field(default_factory=list)
    allowed_surfaces: list[str] = Field(default_factory=list)
    read_receipts: list[QueryReceipt] = Field(default_factory=list)


class JudgeRequest(BaseModel):
    """Payload for `@lv.judge` stages (pairwise/listwise)."""

    model_config = ConfigDict(frozen=True, extra="forbid")

    kind: Literal["pairwise", "listwise"]
    case_id: str
    candidates: list[str]
    """Candidate ids being compared."""
    rubric: str | None = None
    source_refs: list[StageSourceRef] = Field(default_factory=list)
    read_receipts: list[QueryReceipt] = Field(default_factory=list)


__all__ = [
    "JudgeRequest",
    "ProposeRequest",
    "ReflectExample",
    "ReflectRequest",
    "ReflectionResult",
    "StageSourceRef",
]
