"""Load registered Python stages inside a subprocess worker."""

import runpy
from pathlib import Path
from typing import cast

from ..artifacts.prompt import PromptArtifact
from ..decorators import RegisteredStage
from ..proposal import ProposalBatch
from ..stage_payloads import ProposeRequest
from .stage_types import WorkerStage


def load_stage_from_file(
    module_file: Path,
    *,
    stage_id: str,
    stage_name: str,
) -> WorkerStage:
    """Execute a stage file as a module and return the requested stage."""
    namespace = runpy.run_path(str(module_file), run_name=_run_name(module_file))
    if stage_name in namespace:
        stage = namespace[stage_name]
        if isinstance(stage, RegisteredStage):
            return _worker_stage(stage)
    for value in namespace.values():
        if isinstance(value, RegisteredStage) and value.id == stage_id:
            return _worker_stage(value)
    available = sorted(
        name for name, value in namespace.items() if isinstance(value, RegisteredStage)
    )
    raise LookupError(
        f"stage {stage_id!r} / {stage_name!r} not found in {module_file}; "
        f"available registered stages: {available}"
    )


def _run_name(module_file: Path) -> str:
    stem = module_file.stem.replace("-", "_").replace(".", "_")
    return f"leaven_stage_worker_{stem}"


def _worker_stage(stage: RegisteredStage) -> WorkerStage:
    if stage.role == "runner":
        return cast("RegisteredStage[PromptArtifact, str]", stage)
    if stage.role == "proposer":
        return cast("RegisteredStage[ProposeRequest, ProposalBatch]", stage)
    raise ValueError(f"unsupported worker stage role: {stage.role!r}")
