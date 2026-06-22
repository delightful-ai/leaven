"""Optional Harbor adapter for Leaven task, rollout, reward, and evidence glue."""

import importlib
from pathlib import Path

from leaven.x.harbor._types import (
    CtrfEvidence,
    HarborAdapterError,
    HarborTrialOutcome,
    TokenEvidence,
)


def materialize_agent_kit(kit: object, target_dir: Path) -> Path:
    from leaven.x.harbor._kit import materialize_agent_kit as impl  # noqa: PLC0415

    return impl(kit, target_dir)


def task(path: str | Path, *, split: str = "train", id_prefix: str = "harbor") -> object:
    from leaven.x.harbor._task import task as impl  # noqa: PLC0415

    return impl(path, split=split, id_prefix=id_prefix)


def case_from_task_dir(
    path: str | Path, *, split: str = "train", id_prefix: str = "harbor"
) -> object:
    from leaven.x.harbor._task import case_from_task_dir as impl  # noqa: PLC0415

    return impl(path, split=split, id_prefix=id_prefix)


def trajectory_excerpt(
    path: str | Path | None, *, max_steps: int = 4, strict: bool = False
) -> str:
    from leaven.x.harbor._trajectory import trajectory_excerpt as impl  # noqa: PLC0415

    return impl(path, max_steps=max_steps, strict=strict)


def import_trial_result(path: str | Path) -> HarborTrialOutcome:
    """Import a Harbor trial directory into the structured Leaven outcome model."""
    trial_dir = Path(path)
    outcome_path = trial_dir / "leaven_outcome.json"
    if outcome_path.is_file():
        return HarborTrialOutcome.decode(outcome_path.read_text(encoding="utf-8"))
    raise HarborAdapterError(
        f"cannot import Harbor trial result from {trial_dir}: "
        "expected leaven_outcome.json in this adapter slice"
    )


def __getattr__(name: str) -> object:
    if name in {"rewards", "rollout"}:
        module = importlib.import_module(f"leaven.x.harbor.{name}")
        globals()[name] = module
        return module
    if name in {"DEFAULT_WORKDIR", "SKILLS_SUBDIR", "LeavenCodex"}:
        from leaven.x.harbor._agent import (  # noqa: PLC0415
            DEFAULT_WORKDIR,
            SKILLS_SUBDIR,
            LeavenCodex,
        )

        values = {
            "DEFAULT_WORKDIR": DEFAULT_WORKDIR,
            "SKILLS_SUBDIR": SKILLS_SUBDIR,
            "LeavenCodex": LeavenCodex,
        }
        return values[name]
    raise AttributeError(name)


__all__ = [
    "CtrfEvidence",
    "HarborAdapterError",
    "HarborTrialOutcome",
    "TokenEvidence",
    "case_from_task_dir",
    "import_trial_result",
    "materialize_agent_kit",
    "rewards",
    "rollout",
    "task",
    "trajectory_excerpt",
]
