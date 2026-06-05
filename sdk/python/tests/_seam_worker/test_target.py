"""Tests for private stage worker targets."""

import pytest

import leaven as lv
from leaven._seam_worker import worker_argv_for_stage
from leaven.decorators import RegisteredStage


def test_worker_argv_rejects_callable_object_stage() -> None:
    """Boundary check: subprocess workers reload functions by module/name."""

    class CallableRunner:
        async def __call__(
            self,
            prompt: lv.PromptArtifact,
            case: lv.InputCaseView,
            cx: lv.RolloutContext,
        ) -> str:
            _ = (prompt, case, cx)
            return "ok"

    stage: RegisteredStage[lv.PromptArtifact, str] = RegisteredStage(
        role="runner",
        id="callable.runner",
        func=CallableRunner(),
    )

    with pytest.raises(TypeError, match="stage workers require function-backed registered stages"):
        worker_argv_for_stage(stage)
