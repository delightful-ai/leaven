"""Map local Harbor task packages into target-free Leaven tasks/cases."""

import hashlib
import tomllib
from pathlib import Path

import leaven as lv


def task(path: str | Path, *, split: str = "train", id_prefix: str = "harbor") -> lv.Task:
    """Read a local Harbor task package into a one-case Leaven Task."""
    case = case_from_task_dir(path, split=split, id_prefix=id_prefix)
    return lv.Task(
        name=str(case.metadata.get("task_name") or case.id),
        cases=[case],
        metadata={
            "source": "harbor",
            "task_path": case.metadata["task_path"],
            "task_checksum": case.metadata["task_checksum"],
        },
    )


def case_from_task_dir(
    path: str | Path, *, split: str = "train", id_prefix: str = "harbor"
) -> lv.Case:
    """Build a target-free Leaven case for a local Harbor task directory."""
    task_dir = Path(path)
    metadata = _task_metadata(task_dir)
    slug = _slug(str(metadata.get("name") or task_dir.name))
    checksum = _task_checksum(task_dir)
    return lv.Case(
        id=f"{id_prefix}_{slug}_{split}",
        input={"harbor_task": {"path": str(task_dir), "kind": "local"}},
        target=None,
        metadata={
            "source": "harbor",
            "task_name": str(metadata.get("name") or task_dir.name),
            "task_path": str(task_dir),
            "task_checksum": checksum,
        },
        split=split,
    )


def _task_metadata(task_dir: Path) -> dict[str, object]:
    task_toml = task_dir / "task.toml"
    if not task_toml.is_file():
        return {"name": task_dir.name}
    data = tomllib.loads(task_toml.read_text(encoding="utf-8"))
    return data if isinstance(data, dict) else {"name": task_dir.name}


def _task_checksum(task_dir: Path) -> str:
    hasher = hashlib.sha256()
    for file in sorted(task_dir.rglob("*")):
        if not file.is_file() or _is_private_harbor_path(file.relative_to(task_dir)):
            continue
        hasher.update(file.relative_to(task_dir).as_posix().encode())
        hasher.update(b"\0")
        hasher.update(file.read_bytes())
        hasher.update(b"\0")
    return hasher.hexdigest()


def _is_private_harbor_path(relative: Path) -> bool:
    parts = set(relative.parts)
    return bool(parts.intersection({"solution", "solutions", "verifier", "tests"})) or (
        relative.name.startswith("verifier")
    )


def _slug(value: str) -> str:
    lowered = value.strip().lower()
    chars = [ch if ch.isalnum() else "_" for ch in lowered]
    return "_".join("".join(chars).split("_")).strip("_") or "task"


__all__ = ["case_from_task_dir", "task"]
