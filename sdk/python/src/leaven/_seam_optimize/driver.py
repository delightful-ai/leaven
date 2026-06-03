"""Durable public-seam optimize mechanics driver."""

from __future__ import annotations

import asyncio
from typing import Any

from .._seam import (
    MockRunnerStageConfig,
    SeamClient,
    SeamExecutionContext,
    SeamServiceConfig,
    StageRunRequest,
)
from ..artifacts.prompt import PromptArtifact
from ..lm.mock import MockLm
from ..runtime import Runtime
from .scoring import exact_answer_score, mean_score
from .types import SeamOptimizeReport, SeamStageAssessment


async def run_prompt_mechanics(
    *,
    seed: PromptArtifact,
    cases: list[dict[str, Any]],
    run_id: str,
    runtime: Runtime,
) -> SeamOptimizeReport:
    """Run the current prompt slice through durable `leaven/stage.run`.

    This is mechanics evidence for the durable server route. The configured
    stage runner is deterministic and service-side; it is not Python-authored
    worker execution and does not perform optimizer search.
    """
    runner_text = _runner_text(runtime)
    client = SeamClient(
        config=SeamServiceConfig(
            context=SeamExecutionContext(
                capability_fingerprint="fp_cap_sha256_python_optimize",
                policy_fingerprint="fp_policy_sha256_python_optimize",
                base_revision=f"rev_{run_id}",
            ),
            stage=MockRunnerStageConfig(
                text=runner_text,
                summary="deterministic durable seam runner output",
            ),
        )
    )
    assessments = []
    for index, case in enumerate(cases):
        result = await asyncio.to_thread(
            client.request,
            StageRunRequest(
                request_id=f"stage-optimize-{index}",
                run_id=f"run_{run_id}",
                stage_call_id=f"sc_{run_id}_{index}",
                candidate="cand_seed",
                case=case["case_id"],
                case_input=_case_input(seed, case),
            ).to_json_rpc(),
        )
        output = result["output"]["value"]
        assessments.append(
            SeamStageAssessment(
                case_id=case["case_id"],
                output=output,
                score=exact_answer_score(output, case.get("target")),
            )
        )
    score = mean_score([assessment.score for assessment in assessments])
    return SeamOptimizeReport(seed_score=score, best_score=score, assessments=assessments)


def _case_input(seed: PromptArtifact, case: dict[str, Any]) -> dict[str, Any]:
    value = dict(case["input"])
    try:
        value["prompt"] = seed.template.format(**value)
    except KeyError:
        value["prompt"] = seed.template
    return value


def _runner_text(runtime: Runtime) -> str:
    lm = runtime.lm
    if isinstance(lm, MockLm) and lm.responses:
        return lm.responses[0]
    if isinstance(lm, list):
        for config in lm:
            if isinstance(config, MockLm) and config.responses:
                return config.responses[0]
    if isinstance(lm, dict):
        for config in lm.values():
            if isinstance(config, MockLm) and config.responses:
                return config.responses[0]
    return "[mock]"


__all__ = ["run_prompt_mechanics"]
