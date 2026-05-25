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
