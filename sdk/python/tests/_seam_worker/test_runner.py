"""Tests for private runner stage execution records."""

from leaven._seam._wire.payloads import RunnerRequest, StageRunRequest, StageRunResult
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
    assert prompt.template == "Say ok."
    assert case.input == {"extra": "value"}
    return " ok "


async def test_runner_stage_returns_generated_stage_run_result() -> None:
    """Scenario: runner execution keeps typed stage-result records until stdout."""

    stage: RegisteredStage[PromptArtifact, str] = RegisteredStage(
        role="runner",
        id="runner_worker.run",
        func=_run_prompt,
    )

    result = await run_runner_stage(
        stage,
        StageRunRequest(
            schema_version="leaven.stage_run.v1",
            message="stage_run_request",
            stage="runner",
            payload=RunnerRequest(
                schema_version="leaven.stage_payloads.v1",
                run="run_runner_worker",
                stage_call_id="sc_runner_worker",
                candidate="cand_runner_worker",
                case="case_runner_worker",
                case_input={"prompt": "Say ok.", "extra": "value"},
                target_forbidden=True,
                capability_fingerprint="fp_cap_sha256_runner_worker",
            ),
        ),
        lm_model="mock",
    )

    assert isinstance(result, StageRunResult)
    assert result.stage == "runner"
    assert result.output.kind == "text"
    assert result.output.value == "ok"
    assert result.effect_receipts == []
