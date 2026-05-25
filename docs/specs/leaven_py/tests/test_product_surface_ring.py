from __future__ import annotations

import leaven as lv


def test_top_level_leaven_surface_is_product_only() -> None:
    """Product imports should not teach wire, receipt, or engine proof nouns."""

    forbidden = {
        "AssessmentWrite",
        "CallReceipt",
        "EvalContext",
        "EvaluationItem",
        "EvaluationJob",
        "EvidenceEnvelope",
        "EvidencePrivate",
        "EvidencePublic",
        "Environment",
        "OutputRecord",
        "ProposalBatch",
        "ProposalEffect",
        "ProposeRequest",
        "QueryReceipt",
        "ReflectExample",
        "ReflectRequest",
        "ReflectionResult",
        "RegisteredStage",
        "RunCase",
        "RunContext",
        "ScoreCase",
        "StageContext",
        "StageRole",
        "StageSourceRef",
        "Visibility",
        "WorkspaceHandle",
        "WorkspaceLifetime",
        "WorkspaceSurface",
        "WriteReceipt",
        "environment",
    }

    assert forbidden.isdisjoint(lv.__all__)


def test_product_surface_keeps_small_user_vocabulary() -> None:
    """The ordinary API is the artifact-task-stages-runtime vocabulary."""

    expected = {
        "Case",
        "PromptArtifact",
        "Runtime",
        "Score",
        "SkillBank",
        "Stages",
        "Task",
        "evolve",
        "runtime",
    }

    assert expected <= set(lv.__all__)


def test_data_class_surface_has_no_pre_publish_compatibility_aliases() -> None:
    """Data classes should expose locked seam names, not draft aliases."""

    forbidden = {"ARTIFACT_OUTPUT", "TRACE_ONLY"}

    assert forbidden.isdisjoint(lv.data_class.__all__)
    assert lv.data_class.CANDIDATE_ARTIFACT == "candidate.artifact"
    assert lv.data_class.TRANSCRIPT_RAW == "transcript.raw"
