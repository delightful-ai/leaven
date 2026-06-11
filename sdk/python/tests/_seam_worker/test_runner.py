"""Tests for private runner stage execution records."""

import pytest

from leaven._seam._wire.payloads import RunnerRequest, StageRunRequest, StageRunResult
from leaven._seam._wire.refs import CaseInputPayload
from leaven._seam_worker.runner import run_runner_stage
from leaven.artifacts.prompt import PromptArtifact
from leaven.case import InputCaseView
from leaven.contexts import RolloutContext
from leaven.decorators import RegisteredStage


async def _run_prompt(
    prompt: PromptArtifact,
    case: InputCaseView,
    cx: RolloutContext,
) -> str:
    _ = cx
    assert prompt.template == "Say {extra}."
    assert case.input == {"extra": "value"}
    return f" {prompt.template.format(**case.input)} "


def _runner_request(case_input: CaseInputPayload) -> StageRunRequest:
    return StageRunRequest(
        schema_version="leaven.stage_run.v1",
        message="stage_run_request",
        stage="runner",
        payload=RunnerRequest(
            schema_version="leaven.stage_payloads.v1",
            run="run_runner_worker",
            stage_call_id="sc_runner_worker",
            candidate="cand_runner_worker",
            case="case_runner_worker",
            case_input=case_input,
            target_forbidden=True,
            capability_fingerprint="fp_cap_sha256_runner_worker",
        ),
    )


def _stage() -> RegisteredStage[PromptArtifact, str]:
    return RegisteredStage(role="runner", id="runner_worker.run", func=_run_prompt)


async def test_runner_stage_returns_generated_stage_run_result() -> None:
    """Scenario: runner runs the candidate template against the case input."""

    result = await run_runner_stage(
        _stage(),
        _runner_request(
            {"candidate_template": "Say {extra}.", "case_input": {"extra": "value"}}
        ),
        lm_model="mock",
    )

    assert isinstance(result, StageRunResult)
    assert result.stage == "runner"
    assert result.output.kind == "text"
    assert result.output.value == "Say value."
    assert result.effect_receipts == []


async def test_runner_stage_rejects_missing_candidate_template() -> None:
    """Boundary check: the runner payload must carry the candidate template."""

    with pytest.raises(ValueError, match="runner case_input must carry candidate_template"):
        await run_runner_stage(
            _stage(),
            _runner_request({"case_input": {"extra": "value"}}),
            lm_model="mock",
        )


async def test_runner_stage_rejects_non_text_candidate_template() -> None:
    """Boundary check: the candidate template must be a string, not coerced."""

    with pytest.raises(TypeError, match=r"runner case_input\.candidate_template must be a string"):
        await run_runner_stage(
            _stage(),
            _runner_request({"candidate_template": 12, "case_input": {"extra": "value"}}),
            lm_model="mock",
        )
