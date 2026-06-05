"""Tests for public stage payload records."""

import pytest
from pydantic import ValidationError

from leaven.stage_payloads import (
    ReflectionFailureMode,
    ReflectionResult,
    ReflectionSurfaceSuggestion,
    StageSourceRef,
)


def test_reflection_result_carries_typed_diagnostics() -> None:
    """Example: reflection diagnostics are fields, not a metadata object bag."""

    source = StageSourceRef(kind="candidate", id="cand_parent")
    result = ReflectionResult(
        diagnosis="empty answers fail",
        diagnosis_source_refs=[source],
        failure_modes=[
            ReflectionFailureMode(
                label="empty_answer",
                description="candidate returned no answer",
                severity="blocking",
                source_refs=[source],
            )
        ],
        surface_suggestions=[
            ReflectionSurfaceSuggestion(
                surface_fingerprint="fp_surface_sha256_prompt",
                diagnosis="edit prompt instructions",
                part_label="instructions",
                suggested_direction="ask for a final numeric answer",
                constraints=["keep examples hidden"],
                source_refs=[source],
            )
        ],
        negative_constraints=["do not reveal target"],
        positive_constraints=["cite source refs"],
        confidence=0.8,
    )

    assert result.failure_modes[0].source_refs == [source]
    assert result.surface_suggestions[0].constraints == ["keep examples hidden"]
    assert result.negative_constraints == ["do not reveal target"]
    assert result.confidence == 0.8


def test_reflection_result_rejects_metadata_bag() -> None:
    """Regression: reflection metadata is no longer arbitrary JSON."""

    with pytest.raises(ValidationError, match="Extra inputs are not permitted"):
        ReflectionResult.model_validate({"diagnosis": "bad", "metadata": {"failure_modes": []}})
