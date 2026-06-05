"""Command-line entrypoint for the private command-runner worker."""

import argparse
import asyncio
from pathlib import Path
from typing import cast

from .._seam._wire import JsonRpcId
from .._seam._wire.payloads import StageRunRequest, StageRunResult
from ..artifacts.prompt import PromptArtifact
from ..decorators import RegisteredStage
from ..proposal import ProposalBatch
from ..stage_payloads import ProposeRequest
from .loader import load_stage_from_file
from .proposer import run_proposer_stage
from .protocol import read_request, write_error, write_result
from .runner import run_runner_stage
from .stage_types import WorkerStage


def main(argv: list[str] | None = None) -> int:
    """Run one stage.run request from stdin and write one JSON-RPC response."""
    args = _parser().parse_args(argv)
    request_id: JsonRpcId = None
    try:
        stage = load_stage_from_file(
            args.module_file,
            stage_id=args.stage_id,
            stage_name=args.stage_name,
        )
        request = read_request()
        request_id = request.request_id
        result = asyncio.run(run_stage(stage, request.params, lm_model=args.lm_model))
    except Exception as error:
        write_error(request_id, str(error))
        return 1
    else:
        write_result(request, result)
        return 0


def _parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(prog="python -m leaven._seam_worker")
    parser.add_argument("--module-file", required=True, type=Path)
    parser.add_argument("--stage-id", required=True)
    parser.add_argument("--stage-name", required=True)
    parser.add_argument("--lm-model", required=True)
    return parser


async def run_stage(
    stage: WorkerStage,
    params: StageRunRequest,
    *,
    lm_model: str,
) -> StageRunResult:
    """Dispatch one registered stage by role."""
    if stage.role == "runner":
        return await run_runner_stage(
            _runner_stage(stage),
            params,
            lm_model=lm_model,
        )
    if stage.role == "proposer":
        return await run_proposer_stage(
            _proposer_stage(stage),
            params,
            lm_model=lm_model,
        )
    raise ValueError(f"unsupported worker stage role: {stage.role!r}")


def _runner_stage(stage: WorkerStage) -> RegisteredStage[PromptArtifact, str]:
    if stage.role != "runner":
        raise TypeError(f"worker stage is not a runner: {stage.role!r}")
    return cast("RegisteredStage[PromptArtifact, str]", stage)


def _proposer_stage(stage: WorkerStage) -> RegisteredStage[ProposeRequest, ProposalBatch]:
    if stage.role != "proposer":
        raise TypeError(f"worker stage is not a proposer: {stage.role!r}")
    return cast("RegisteredStage[ProposeRequest, ProposalBatch]", stage)


__all__ = ["main", "run_stage"]
