#!/usr/bin/env python3
"""Check that the deterministic one-case result JSONL matches current artifacts."""

from __future__ import annotations

import argparse
import subprocess
import tempfile
from pathlib import Path


RESULT_JSONL = Path("docs/ara/trace2skill_spreadsheetbench/results/deterministic_one_case.jsonl")
RUN_DIR = Path("tmp/trace2skill-one-case-live")


def build_result(repo_root: Path, output: Path) -> subprocess.CompletedProcess[str]:
    command = [
        "uv",
        "run",
        "python",
        str(repo_root / "scripts/build_trace2skill_one_case_result.py"),
        "--run-dir",
        str(RUN_DIR),
        "--output",
        str(output),
    ]
    return subprocess.run(command, cwd=repo_root, text=True, capture_output=True, check=False)


def check_one_case_result_freshness(repo_root: Path, ara_root: Path) -> list[str]:
    del ara_root
    errors: list[str] = []
    committed = repo_root / RESULT_JSONL
    if not committed.is_file():
        return [f"missing committed deterministic one-case result: {RESULT_JSONL}"]

    with tempfile.TemporaryDirectory(prefix="trace2skill-one-case-result-") as temp:
        rendered = Path(temp) / "deterministic_one_case.jsonl"
        result = build_result(repo_root, rendered)
        if result.returncode != 0:
            detail = "\n".join(part for part in [result.stdout.strip(), result.stderr.strip()] if part)
            return [f"one-case result regeneration failed with exit {result.returncode}: {detail}"]
        if not rendered.is_file():
            return [f"one-case result builder did not create expected temp file: {rendered}"]
        committed_text = committed.read_text(encoding="utf-8")
        rendered_text = rendered.read_text(encoding="utf-8")
        if committed_text != rendered_text:
            errors.append(
                f"{RESULT_JSONL} is stale; regenerate with "
                "uv run python scripts/build_trace2skill_one_case_result.py"
            )
    return errors


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("ara_dir", type=Path)
    args = parser.parse_args()

    ara_root = args.ara_dir.resolve()
    repo_root = Path(__file__).resolve().parents[1]
    errors = check_one_case_result_freshness(repo_root, ara_root)
    if errors:
        for error in errors:
            print(f"ERROR: {error}")
        return 1
    print(f"PASS: {args.ara_dir} deterministic one-case result freshness")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
