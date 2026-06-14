"""Typed Trace2Skill reproduction skeleton.

This module documents the algorithmic shape the ARA expects. It is not a paper
reproduction runner until connected to real upstream/Leaven execution surfaces.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from enum import StrEnum
from pathlib import Path
from typing import Iterable, Sequence


class Denominator(StrEnum):
    MECHANICS = "mechanics"
    ONE_CASE = "one_case"
    SUBSET = "subset"
    HELD_OUT_200_400 = "held_out_200_400"
    SEED_AGGREGATE_41_42_43 = "seed_aggregate_41_42_43"
    CROSS_MODEL = "cross_model"
    FULL_PAPER = "full_paper"


@dataclass(frozen=True)
class SkillDirectory:
    root: Path


@dataclass(frozen=True)
class Trajectory:
    task_id: str
    success: bool
    transcript_path: Path
    artifact_paths: tuple[Path, ...] = ()


@dataclass(frozen=True)
class SkillPatch:
    patch_id: str
    source_trajectory_id: str
    patch_path: Path
    support_count: int = 1


@dataclass(frozen=True)
class LeavenResult:
    run_id: str
    denominator: Denominator
    dataset_slice: str
    model_id: str
    seed: int | None
    skill_source: str
    metric_name: str
    metric_value: float
    source_command: str
    artifacts: tuple[Path, ...] = field(default_factory=tuple)


def partition_trajectories(trajectories: Iterable[Trajectory]) -> tuple[list[Trajectory], list[Trajectory]]:
    failures: list[Trajectory] = []
    successes: list[Trajectory] = []
    for trajectory in trajectories:
        if trajectory.success:
            successes.append(trajectory)
        else:
            failures.append(trajectory)
    return failures, successes


def propose_error_patch(skill: SkillDirectory, trajectory: Trajectory) -> SkillPatch:
    raise NotImplementedError("Connect to Trace2Skill error analyst or faithful replay artifacts.")


def propose_success_patch(skill: SkillDirectory, trajectory: Trajectory) -> SkillPatch:
    raise NotImplementedError("Connect to Trace2Skill success analyst or faithful replay artifacts.")


def merge_patch_level(skill: SkillDirectory, patches: Sequence[SkillPatch], batch_size: int) -> list[SkillPatch]:
    if batch_size <= 0:
        raise ValueError("batch_size must be positive")
    raise NotImplementedError("Connect to Trace2Skill merge operator or Leaven SkillPatchMergeTree.")


def apply_final_patch(skill: SkillDirectory, patch: SkillPatch) -> SkillDirectory:
    raise NotImplementedError("Connect to Leaven SkillPatchApplication or upstream patch application.")


def evolve_skill(skill: SkillDirectory, trajectories: Sequence[Trajectory], merge_batch_size: int) -> SkillDirectory:
    failures, successes = partition_trajectories(trajectories)
    patches = [propose_error_patch(skill, t) for t in failures]
    patches.extend(propose_success_patch(skill, t) for t in successes)
    level = patches
    while len(level) > 1:
        level = merge_patch_level(skill, level, merge_batch_size)
    if not level:
        return skill
    return apply_final_patch(skill, level[0])
