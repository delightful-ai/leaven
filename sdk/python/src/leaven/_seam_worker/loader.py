"""Load registered Python stages inside a subprocess worker."""

import runpy
from pathlib import Path
from typing import cast

from ..artifacts.prompt import PromptArtifact
from ..decorators import RegisteredStage
from ..proposal import ProposalBatch
from ..rubric import RegisteredReward, Rubric
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


def load_rubric_from_file(module_file: Path, *, reward_ids: list[str]) -> Rubric:
    """Execute a stage file and rebuild the rubric from the requested rewards.

    The optimize host dispatches scorer stages to the same worker argv as runner
    stages, so the worker reloads the module and collects the rubric's
    `@lv.reward` registrations by stable `RegisteredReward.id` (not
    `func.__name__`, which collides across imports and factory wrappers), in the
    order the driver recorded them. A missing reward is a hard error rather than
    a silently smaller rubric. Distinct rewards that share an id refuse reload
    instead of silently overwriting each other.
    """
    namespace = runpy.run_path(str(module_file), run_name=_run_name(module_file))
    by_id: dict[str, RegisteredReward] = {}
    for value in namespace.values():
        if not isinstance(value, RegisteredReward):
            continue
        existing = by_id.get(value.id)
        if existing is not None and existing is not value:
            raise ValueError(
                f"duplicate reward id {value.id!r} in {module_file}; "
                "worker reload keys rewards by id, so colliding ids would "
                "silently replace a dimension of the rubric vector"
            )
        by_id[value.id] = value
    rewards: list[RegisteredReward] = []
    for reward_id in reward_ids:
        if reward_id not in by_id:
            available = sorted(by_id)
            raise LookupError(
                f"reward {reward_id!r} not found in {module_file}; "
                f"available rewards: {available}"
            )
        rewards.append(by_id[reward_id])
    if not rewards:
        raise ValueError("scorer worker requires at least one rubric reward")
    return Rubric(rewards)


def _run_name(module_file: Path) -> str:
    stem = module_file.stem.replace("-", "_").replace(".", "_")
    return f"leaven_stage_worker_{stem}"


def _worker_stage(stage: RegisteredStage) -> WorkerStage:
    if stage.role == "runner":
        return cast("RegisteredStage[PromptArtifact, str]", stage)
    if stage.role == "proposer":
        return cast("RegisteredStage[ProposeRequest, ProposalBatch]", stage)
    raise ValueError(f"unsupported worker stage role: {stage.role!r}")
