"""Tests for private runner stage execution records."""

import pytest

from leaven._seam._wire.payloads import RunnerRequest, StageRunRequest, StageRunResult
from leaven._seam._wire.refs import CaseInputPayload
from leaven._seam_worker.runner import run_runner_stage
from leaven.artifacts.agent_kit import AgentKitArtifact
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


async def _run_kit(
    kit: AgentKitArtifact,
    case: InputCaseView,
    cx: RolloutContext,
) -> str:
    _ = cx
    assert isinstance(kit, AgentKitArtifact)
    assert kit.system_prompt == "Use the marker."
    assert [(s.path, s.content) for s in kit.skills] == [("k.md", "skill body")]
    assert case.input == {"extra": "value"}
    return f"kit:{kit.system_prompt}"


def _kit_stage() -> RegisteredStage[AgentKitArtifact, str]:
    return RegisteredStage(role="runner", id="runner_worker.kit", func=_run_kit)


async def test_runner_stage_reconstructs_an_agent_kit_candidate() -> None:
    """Scenario: a `candidate_agent_kit` payload reaches a kit-typed rollout.

    The host projects each kit candidate revision under `candidate_agent_kit`;
    the worker reconstructs the typed `AgentKitArtifact` the registered runner
    consumes, so one runner worker serves both the prompt and kit paths.
    """
    result = await run_runner_stage(
        _kit_stage(),
        _runner_request(
            {
                "candidate_agent_kit": {
                    "system_prompt": "Use the marker.",
                    "skills": [{"path": "k.md", "content": "skill body"}],
                },
                "case_input": {"extra": "value"},
            }
        ),
        lm_model="mock",
    )

    assert isinstance(result, StageRunResult)
    assert result.output.value == "kit:Use the marker."


async def test_runner_stage_rejects_missing_candidate_payload() -> None:
    """Boundary check: the runner payload must carry a typed candidate."""

    with pytest.raises(
        ValueError,
        match=r"runner case_input must carry a candidate under "
        r"`candidate_template` or `candidate_agent_kit`",
    ):
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
