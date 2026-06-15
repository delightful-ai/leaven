#!/usr/bin/env python3
"""Build a deterministic manifest for the Trace2Skill SpreadsheetBench data."""

from __future__ import annotations

import argparse
import hashlib
import json
from collections import Counter
from pathlib import Path
from typing import Any


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def stable_json(value: Any) -> bytes:
    return json.dumps(value, sort_keys=True, separators=(",", ":")).encode("utf-8")


def resolve_spreadsheet_dir(data_root: Path, spreadsheet_path: str) -> Path:
    direct = data_root / spreadsheet_path
    if direct.exists():
        return direct
    return data_root / "all_data_912_v0.1" / spreadsheet_path


def split_summary(name: str, records: list[dict[str, Any]], start: int, end: int) -> dict[str, Any]:
    ids = [str(record["id"]) for record in records[start:end]]
    return {
        "name": name,
        "range": f"{start}..{end}",
        "case_count": len(ids),
        "first_id": ids[0],
        "last_id": ids[-1],
        "case_order_sha256": sha256_bytes(stable_json(ids)),
    }


def build_manifest(data_root: Path, dataset_rel: Path) -> dict[str, Any]:
    dataset_dir = data_root / dataset_rel
    dataset_json = dataset_dir / "dataset.json"
    records = json.loads(dataset_json.read_text(encoding="utf-8"))
    if not isinstance(records, list):
        raise ValueError(f"{dataset_json} must contain a JSON array")
    if len(records) != 400:
        raise ValueError(f"{dataset_json} must contain 400 records, found {len(records)}")

    case_ids = [str(record["id"]) for record in records]
    key_counts = Counter(key for record in records for key in record)
    instruction_types = Counter(str(record.get("instruction_type", "<missing>")) for record in records)

    workbook_digest = hashlib.sha256()
    workbook_file_count = 0
    workbook_total_bytes = 0
    workbook_files_per_case: list[int] = []
    missing_dirs: list[str] = []

    for record in records:
        spreadsheet_path = str(record["spreadsheet_path"])
        spreadsheet_dir = resolve_spreadsheet_dir(data_root, spreadsheet_path)
        if not spreadsheet_dir.is_dir():
            missing_dirs.append(spreadsheet_path)
            workbook_files_per_case.append(0)
            continue
        files = sorted(path for path in spreadsheet_dir.rglob("*") if path.is_file())
        workbook_files_per_case.append(len(files))
        for path in files:
            rel = path.relative_to(data_root).as_posix()
            file_hash = sha256_file(path)
            workbook_digest.update(rel.encode("utf-8"))
            workbook_digest.update(b"\0")
            workbook_digest.update(file_hash.encode("utf-8"))
            workbook_digest.update(b"\0")
            workbook_file_count += 1
            workbook_total_bytes += path.stat().st_size

    return {
        "schema_version": "leaven.trace2skill.dataset_manifest.v1",
        "source": {
            "data_root": data_root.as_posix(),
            "dataset_path": dataset_rel.as_posix(),
            "dataset_json": (dataset_rel / "dataset.json").as_posix(),
        },
        "dataset_json": {
            "bytes": dataset_json.stat().st_size,
            "sha256": sha256_file(dataset_json),
        },
        "case_count": len(records),
        "case_order": {
            "first_id": case_ids[0],
            "last_id": case_ids[-1],
            "sha256": sha256_bytes(stable_json(case_ids)),
        },
        "splits": [
            split_summary("evolving", records, 0, 200),
            split_summary("held_out", records, 200, 400),
        ],
        "record_shape": {
            "keys_by_presence_count": dict(sorted(key_counts.items())),
            "instruction_type_counts": dict(sorted(instruction_types.items())),
        },
        "referenced_workbooks": {
            "missing_directory_count": len(missing_dirs),
            "missing_directories": missing_dirs,
            "file_count": workbook_file_count,
            "total_bytes": workbook_total_bytes,
            "files_per_case_min": min(workbook_files_per_case),
            "files_per_case_max": max(workbook_files_per_case),
            "aggregate_sha256": workbook_digest.hexdigest(),
        },
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--data-root",
        type=Path,
        default=Path("tmp/repros/trace2skill-upstream/data"),
        help="Trace2Skill upstream data root.",
    )
    parser.add_argument(
        "--dataset-path",
        type=Path,
        default=Path("spreadsheetbench_verified/spreadsheetbench_verified_400"),
        help="Dataset directory path relative to --data-root.",
    )
    parser.add_argument(
        "--output",
        type=Path,
        default=Path("docs/ara/trace2skill_spreadsheetbench/results/dataset_manifest.json"),
    )
    args = parser.parse_args()

    manifest = build_manifest(args.data_root, args.dataset_path)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(args.output)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
