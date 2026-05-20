#!/usr/bin/env python3
"""Materialize tiny official dataset samples for paper-exact replication work."""

from __future__ import annotations

import csv
import hashlib
import json
import os
import shutil
import sys
import zipfile
from pathlib import Path
from typing import Any

import requests


REPO_ROOT = Path(__file__).resolve().parents[1]
OUT_ROOT = REPO_ROOT / "tmp" / "paper_exact_samples"


def clean(value: Any) -> Any:
    if isinstance(value, bytes):
        return {"bytes_len": len(value), "sha256": hashlib.sha256(value).hexdigest()}
    if isinstance(value, dict):
        return {str(k): clean(v) for k, v in value.items()}
    if isinstance(value, list):
        return [clean(v) for v in value]
    if hasattr(value, "item"):
        try:
            return clean(value.item())
        except Exception:
            pass
    return value


def sha256_path(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def write_json(path: Path, data: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(clean(data), indent=2, sort_keys=True) + "\n")


def copy_file(src: Path, dst: Path) -> dict[str, Any]:
    dst.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(src, dst)
    return {
        "path": str(dst.relative_to(REPO_ROOT)),
        "bytes": dst.stat().st_size,
        "sha256": sha256_path(dst),
    }


def first_csv_row(path: Path) -> dict[str, str]:
    with path.open(newline="") as f:
        return next(csv.DictReader(f))


def sample_officeqa() -> dict[str, Any]:
    out = OUT_ROOT / "evoskill" / "officeqa"
    source_csv = REPO_ROOT / "tmp" / "repros" / "officeqa" / "officeqa_pro.csv"
    row = first_csv_row(source_csv)
    write_json(out / "officeqa_pro_first_case.json", row)

    files = {
        "case": {
            "path": str((out / "officeqa_pro_first_case.json").relative_to(REPO_ROOT)),
            "bytes": (out / "officeqa_pro_first_case.json").stat().st_size,
            "sha256": sha256_path(out / "officeqa_pro_first_case.json"),
        }
    }

    zip_path = (
        REPO_ROOT
        / "tmp"
        / "repros"
        / "officeqa"
        / "treasury_bulletins_parsed"
        / "transformed"
        / "treasury_bulletins_transformed.zip"
    )
    source_file = row["source_files"]
    with zipfile.ZipFile(zip_path) as zf:
        with zf.open(source_file) as f:
            text = f.read().decode("utf-8", errors="replace")
    source_dst = out / source_file
    source_dst.write_text(text)
    files["source_text"] = {
        "path": str(source_dst.relative_to(REPO_ROOT)),
        "bytes": source_dst.stat().st_size,
        "sha256": sha256_path(source_dst),
    }

    return {
        "paper": "EvoSkill",
        "dataset": "OfficeQA Pro",
        "status": "materialized",
        "source": str(source_csv.relative_to(REPO_ROOT)),
        "case_id": row.get("uid"),
        "files": files,
    }


def sample_sealqa() -> dict[str, Any]:
    from datasets import load_dataset

    out = OUT_ROOT / "evoskill" / "sealqa"
    ds = load_dataset("vtllms/sealqa", name="seal_0", split="test", streaming=True)
    row = next(iter(ds))
    write_json(out / "seal_0_first_case.json", row)
    path = out / "seal_0_first_case.json"
    return {
        "paper": "EvoSkill",
        "dataset": "SealQA seal_0",
        "status": "materialized",
        "source": "hf://datasets/vtllms/sealqa/seal_0/test",
        "case_id": row.get("id") or row.get("uid"),
        "files": {
            "case": {
                "path": str(path.relative_to(REPO_ROOT)),
                "bytes": path.stat().st_size,
                "sha256": sha256_path(path),
            }
        },
    }


def sample_spreadsheetbench() -> dict[str, Any]:
    out = OUT_ROOT / "trace2skill" / "spreadsheetbench_verified"
    root = (
        REPO_ROOT
        / "tmp"
        / "repros"
        / "trace2skill-upstream"
        / "data"
        / "spreadsheetbench_verified"
        / "spreadsheetbench_verified_400"
    )
    data = json.loads((root / "dataset.json").read_text())
    row = data[0]
    sample_root = root / row["spreadsheet_path"]
    write_json(out / "dataset_first_case.json", row)
    copied = {}
    for src in sorted(sample_root.iterdir()):
        if src.is_file():
            copied[src.name] = copy_file(src, out / sample_root.name / src.name)
    case_path = out / "dataset_first_case.json"
    return {
        "paper": "Trace2Skill",
        "dataset": "SpreadsheetBench Verified 400",
        "status": "materialized",
        "source": str(root.relative_to(REPO_ROOT)),
        "case_id": row.get("id"),
        "files": {
            "case": {
                "path": str(case_path.relative_to(REPO_ROOT)),
                "bytes": case_path.stat().st_size,
                "sha256": sha256_path(case_path),
            },
            "spreadsheet_dir": copied,
        },
    }


def sample_memento_hf(repo_id: str, split: str, out_name: str) -> dict[str, Any]:
    from datasets import load_dataset

    out = OUT_ROOT / "memento-skills" / out_name
    try:
        ds = load_dataset(repo_id, split=split, streaming=True)
        row = next(iter(ds))
    except Exception as exc:  # Hugging Face gates raise several exception types.
        blocked = {
            "paper": "Memento-Skills",
            "dataset": repo_id,
            "status": "blocked",
            "reason": f"{type(exc).__name__}: {exc}",
        }
        write_json(out / "access_blocked.json", blocked)
        return blocked | {
            "files": {
                "access_blocked": {
                    "path": str((out / "access_blocked.json").relative_to(REPO_ROOT)),
                    "bytes": (out / "access_blocked.json").stat().st_size,
                    "sha256": sha256_path(out / "access_blocked.json"),
                }
            }
        }

    write_json(out / "first_case.json", row)
    path = out / "first_case.json"
    return {
        "paper": "Memento-Skills",
        "dataset": repo_id,
        "status": "materialized",
        "source": f"hf://datasets/{repo_id}/{split}",
        "case_id": row.get("id") or row.get("task_id"),
        "files": {
            "case": {
                "path": str(path.relative_to(REPO_ROOT)),
                "bytes": path.stat().st_size,
                "sha256": sha256_path(path),
            }
        },
    }


def download(url: str, dst: Path) -> Path:
    dst.parent.mkdir(parents=True, exist_ok=True)
    if dst.exists():
        return dst
    with requests.get(url, stream=True, timeout=60) as r:
        r.raise_for_status()
        with dst.open("wb") as f:
            for chunk in r.iter_content(chunk_size=1024 * 1024):
                if chunk:
                    f.write(chunk)
    return dst


def sample_alfworld_for_d2skill() -> dict[str, Any]:
    out = OUT_ROOT / "d2skill" / "alfworld"
    zip_path = out / "json_2.1.1_json.zip"
    url = "https://github.com/alfworld/alfworld/releases/download/0.2.2/json_2.1.1_json.zip"
    download(url, zip_path)
    with zipfile.ZipFile(zip_path) as zf:
        candidates = [n for n in zf.namelist() if n.endswith("/traj_data.json")]
        chosen = sorted(candidates)[0]
        row = json.loads(zf.read(chosen).decode("utf-8", errors="replace"))
    write_json(out / "first_traj_data.json", row)
    case_path = out / "first_traj_data.json"

    d2skill_prompt = (
        REPO_ROOT
        / "tmp"
        / "repros"
        / "d2skill-agenticrl"
        / "agent_system"
        / "environments"
        / "prompts"
        / "alfworld.py"
    )
    prompt_copy = copy_file(d2skill_prompt, out / "d2skill_alfworld_prompt_source.py")
    return {
        "paper": "D2Skill",
        "dataset": "ALFWorld json_2.1.1",
        "status": "materialized",
        "source": url,
        "case_id": chosen,
        "files": {
            "case": {
                "path": str(case_path.relative_to(REPO_ROOT)),
                "bytes": case_path.stat().st_size,
                "sha256": sha256_path(case_path),
            },
            "source_zip": {
                "path": str(zip_path.relative_to(REPO_ROOT)),
                "bytes": zip_path.stat().st_size,
                "sha256": sha256_path(zip_path),
            },
            "d2skill_prompt_source": prompt_copy,
        },
    }


def sample_skillsbench_for_skillreducer() -> dict[str, Any]:
    out = OUT_ROOT / "skillreducer" / "skillsbench"
    root = REPO_ROOT / "tmp" / "repros" / "skillsbench"
    task = root / "tasks" / "jax-computing-basics"
    copied = {}
    for rel in [
        Path("task.toml"),
        Path("instruction.md"),
        Path("environment/problem.json"),
        Path("tests/test_outputs.py"),
    ]:
        copied[str(rel)] = copy_file(task / rel, out / "task" / rel)
    skill = root / ".agents" / "skills" / "skill-creator" / "SKILL.md"
    copied["skill-creator/SKILL.md"] = copy_file(skill, out / "skills" / "skill-creator" / "SKILL.md")
    return {
        "paper": "SkillReducer",
        "dataset": "SkillsBench task plus skill fixture",
        "status": "materialized",
        "source": str(root.relative_to(REPO_ROOT)),
        "case_id": "jax-computing-basics",
        "files": copied,
    }


def main() -> int:
    OUT_ROOT.mkdir(parents=True, exist_ok=True)
    entries = []
    for fn in [
        sample_officeqa,
        sample_sealqa,
        sample_spreadsheetbench,
        lambda: sample_memento_hf("gaia-benchmark/GAIA", "validation", "gaia"),
        lambda: sample_memento_hf("cais/hle", "test", "hle"),
        sample_alfworld_for_d2skill,
        sample_skillsbench_for_skillreducer,
    ]:
        try:
            entries.append(fn())
        except Exception as exc:
            entries.append(
                {
                    "status": "error",
                    "function": getattr(fn, "__name__", "lambda"),
                    "reason": f"{type(exc).__name__}: {exc}",
                }
            )
    manifest = {
        "output_root": str(OUT_ROOT.relative_to(REPO_ROOT)),
        "entries": entries,
    }
    write_json(OUT_ROOT / "manifest.json", manifest)
    print(json.dumps(manifest, indent=2, sort_keys=True))
    return 0 if all(e.get("status") in {"materialized", "blocked"} for e in entries) else 1


if __name__ == "__main__":
    sys.exit(main())
