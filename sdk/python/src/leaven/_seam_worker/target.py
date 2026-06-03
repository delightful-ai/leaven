"""Serializable worker target for registered Python stages."""

from __future__ import annotations

import inspect
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any


@dataclass(frozen=True)
class StageWorkerTarget:
    """How a subprocess worker re-loads a registered stage."""

    stage_id: str
    stage_name: str
    module_file: Path

    def argv(self, *, lm_model: str = "mock") -> tuple[str, ...]:
        """Return the command used by `CommandRunnerStageConfig`."""
        return (
            sys.executable,
            "-m",
            "leaven._seam_worker",
            "--module-file",
            str(self.module_file),
            "--stage-id",
            self.stage_id,
            "--stage-name",
            self.stage_name,
            "--lm-model",
            lm_model,
        )


def worker_argv_for_stage(stage: Any, *, lm_model: str = "mock") -> tuple[str, ...]:
    """Build worker argv for a `RegisteredStage` without importing it here."""
    target = StageWorkerTarget(
        stage_id=stage.id,
        stage_name=getattr(stage.func, "__name__", stage.id.rsplit(".", 1)[-1]),
        module_file=_module_file(stage),
    )
    return target.argv(lm_model=lm_model)


def _module_file(stage: Any) -> Path:
    path = inspect.getsourcefile(stage.func) or inspect.getfile(stage.func)
    if not path:
        raise ValueError(f"registered stage {stage.id!r} has no source file")
    return Path(path).resolve()
