import importlib.util

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
        "environment",
    }

    assert forbidden.isdisjoint(lv.__all__)


def test_product_surface_keeps_small_user_vocabulary() -> None:
    """The ordinary API is the seed-environment-optimizer-runtime vocabulary."""

    expected = {
        "Case",
        "Environment",
        "OptimizeBuilder",
        "PromptArtifact",
        "Rollout",
        "Rubric",
        "Runtime",
        "Score",
        "SkillBank",
        "Task",
        "optimize",
        "reward",
        "runner",
        "runtime",
    }

    assert expected <= set(lv.__all__)


def test_standalone_worker_loop_is_not_public_until_implemented() -> None:
    """The ordinary SDK must not export scaffold backbone entrypoints."""

    assert "serve_stage" not in lv.__all__
    assert not hasattr(lv, "serve_stage")


def test_dspy_adapter_is_absent_until_it_executes() -> None:
    """Unwired optional adapters must not be exported as public product surface."""

    forbidden = {"dspy_acall", "dspy_call_context", "dspy_context"}

    assert forbidden.isdisjoint(lv.__all__)
    for name in forbidden:
        assert not hasattr(lv, name)
    assert lv.x.__all__ == []
    assert not hasattr(lv.x, "dspy")
    assert importlib.util.find_spec("leaven.x.dspy") is None


def test_optimize_builder_does_not_advertise_unwired_dry_run() -> None:
    """Builder methods should name executable product behavior only."""

    assert not hasattr(lv.OptimizeBuilder, "dry_run")


def test_data_class_surface_has_no_pre_publish_compatibility_aliases() -> None:
    """Data classes should expose locked seam names, not draft aliases."""

    forbidden = {"ARTIFACT_OUTPUT", "TRACE_ONLY"}

    assert forbidden.isdisjoint(lv.data_class.__all__)
    assert lv.data_class.CANDIDATE_ARTIFACT == "candidate.artifact"
    assert lv.data_class.TRANSCRIPT_RAW == "transcript.raw"


def test_lm_namespace_exports_message_records_needed_by_public_builder() -> None:
    """Message-based LM calls should not require importing private builder modules."""

    assert "LmMessage" in lv.lm.__all__
    assert lv.lm.LmMessage(role="user", content="say ok").role == "user"
