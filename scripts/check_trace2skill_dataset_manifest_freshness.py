#!/usr/bin/env python3
"""Check that the Trace2Skill dataset manifest matches current source data."""

from __future__ import annotations

import argparse
import subprocess
import tempfile
from pathlib import Path


DATASET_MANIFEST = Path("docs/ara/trace2skill_spreadsheetbench/results/dataset_manifest.json")


def build_manifest(repo_root: Path, output: Path) -> subprocess.CompletedProcess[str]:
    command = [
        "uv",
        "run",
        "python",
        str(repo_root / "scripts/build_trace2skill_dataset_manifest.py"),
        "--output",
        str(output),
    ]
    return subprocess.run(command, cwd=repo_root, text=True, capture_output=True, check=False)


def check_dataset_manifest_freshness(repo_root: Path, ara_root: Path) -> list[str]:
    del ara_root
    committed = repo_root / DATASET_MANIFEST
    if not committed.is_file():
        return [f"missing committed dataset manifest: {DATASET_MANIFEST}"]

    with tempfile.TemporaryDirectory(prefix="trace2skill-dataset-manifest-") as temp:
        rendered = Path(temp) / "dataset_manifest.json"
        result = build_manifest(repo_root, rendered)
        if result.returncode != 0:
            detail = "\n".join(part for part in [result.stdout.strip(), result.stderr.strip()] if part)
            return [f"dataset manifest regeneration failed with exit {result.returncode}: {detail}"]
        if not rendered.is_file():
            return [f"dataset manifest builder did not create expected temp file: {rendered}"]
        committed_text = committed.read_text(encoding="utf-8")
        rendered_text = rendered.read_text(encoding="utf-8")
        if committed_text != rendered_text:
            return [
                f"{DATASET_MANIFEST} is stale; regenerate with "
                "uv run python scripts/build_trace2skill_dataset_manifest.py"
            ]
    return []


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("ara_dir", type=Path)
    args = parser.parse_args()

    ara_root = args.ara_dir.resolve()
    repo_root = Path(__file__).resolve().parents[1]
    errors = check_dataset_manifest_freshness(repo_root, ara_root)
    if errors:
        for error in errors:
            print(f"ERROR: {error}")
        return 1
    print(f"PASS: {args.ara_dir} dataset manifest freshness")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
