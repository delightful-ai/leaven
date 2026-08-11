"""Serializable worker target for registered Python stages."""

import inspect
import sys
from collections.abc import Awaitable, Callable
from dataclasses import dataclass
from pathlib import Path
from types import FunctionType
from typing import Protocol


@dataclass(frozen=True)
class StageWorkerTarget:
    """How a subprocess worker re-loads a registered stage.

    The same worker argv serves both runner and scorer stage dispatch, so it
    carries the runner stage identity plus the rubric reward ids the worker
    rebuilds the rubric from when the optimize host dispatches a scorer stage.
    """

    stage_id: str
    stage_name: str
    module_file: Path
    reward_ids: tuple[str, ...] = ()

    def argv(self, *, lm_model: str = "mock") -> tuple[str, ...]:
        """Return the command used by `CommandRunnerStageConfig`."""
        reward_args: tuple[str, ...] = ()
        for reward_id in self.reward_ids:
            reward_args = (*reward_args, "--rubric-reward", reward_id)
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
            *reward_args,
        )


class _StageLike[O](Protocol):
    id: str
    func: Callable[..., Awaitable[O]]


def worker_argv_for_stage[O](
    stage: _StageLike[O],
    *,
    lm_model: str = "mock",
    reward_ids: tuple[str, ...] = (),
) -> tuple[str, ...]:
    """Build worker argv for a `RegisteredStage` without importing it here.

    `reward_ids` carries the rubric reward ids the worker rebuilds the rubric
    from when the optimize host dispatches a scorer stage to the same argv.
    """
    if not isinstance(stage.func, FunctionType):
        raise TypeError("stage workers require function-backed registered stages")
    target = StageWorkerTarget(
        stage_id=stage.id,
        stage_name=stage.func.__name__,
        module_file=_module_file(stage),
        reward_ids=reward_ids,
    )
    return target.argv(lm_model=lm_model)


def _module_file[O](stage: _StageLike[O]) -> Path:
    path = inspect.getsourcefile(stage.func) or inspect.getfile(stage.func)
    if not path:
        raise ValueError(f"registered stage {stage.id!r} has no source file")
    return Path(path).resolve()
