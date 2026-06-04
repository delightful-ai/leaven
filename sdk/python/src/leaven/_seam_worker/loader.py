"""Load registered Python stages inside a subprocess worker."""

import runpy
from pathlib import Path

from ..decorators import RegisteredStage


def load_stage_from_file(
    module_file: Path,
    *,
    stage_id: str,
    stage_name: str,
) -> RegisteredStage[object, object]:
    """Execute a stage file as a module and return the requested stage."""
    namespace = runpy.run_path(str(module_file), run_name=_run_name(module_file))
    stage = namespace.get(stage_name)
    if isinstance(stage, RegisteredStage):
        return stage
    for value in namespace.values():
        if isinstance(value, RegisteredStage) and value.id == stage_id:
            return value
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
