#!/usr/bin/env python3
"""Validate Trace2Skill ARA status docs against current artifacts."""

from __future__ import annotations

import argparse
import json
from dataclasses import dataclass
from pathlib import Path
from typing import Any


RESULT_SCHEMA_VERSION = "leaven.trace2skill.result.v1"


@dataclass(frozen=True)
class ResultSummary:
    jsonl_files: int
    total_rows: int
    non_overlay_rows: int
    overlay_rows: int
    paper_denominator_rows: int


def repo_root_for(ara_root: Path) -> Path:
    for candidate in (ara_root, *ara_root.parents):
        if (candidate / "docs/ara/trace2skill_spreadsheetbench").is_dir():
            return candidate
    return Path.cwd()


def read(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def ara_file_count(ara_root: Path) -> int:
    return sum(1 for path in ara_root.rglob("*") if path.is_file())


def iter_result_records(ara_root: Path) -> list[dict[str, Any]]:
    records: list[dict[str, Any]] = []
    results_dir = ara_root / "results"
    for jsonl_path in sorted(results_dir.glob("*.jsonl")):
        for line_number, line in enumerate(read(jsonl_path).splitlines(), start=1):
            if not line.strip():
                continue
            try:
                record = json.loads(line)
            except json.JSONDecodeError as exc:
                rel_path = jsonl_path.relative_to(ara_root)
                raise ValueError(f"{rel_path}:{line_number} is invalid JSON: {exc}") from exc
            records.append(record)
    return records


def summarize_results(ara_root: Path) -> ResultSummary:
    results_dir = ara_root / "results"
    jsonl_files = len(list(results_dir.glob("*.jsonl"))) if results_dir.is_dir() else 0
    records = iter_result_records(ara_root)
    non_overlay_rows = sum(1 for record in records if record.get("plot_binding") is None)
    overlay_rows = len(records) - non_overlay_rows
    paper_denominator_rows = sum(
        1
        for record in records
        if record.get("schema_version") == RESULT_SCHEMA_VERSION
        and record.get("proof_classification") == "paper-denominator-reproduction"
    )
    return ResultSummary(
        jsonl_files=jsonl_files,
        total_rows=len(records),
        non_overlay_rows=non_overlay_rows,
        overlay_rows=overlay_rows,
        paper_denominator_rows=paper_denominator_rows,
    )


def check_closeout_summary(ara_root: Path, summary: ResultSummary) -> list[str]:
    errors: list[str] = []
    closeout_path = ara_root / "results/closeout_audit.json"
    if not closeout_path.is_file():
        return ["missing results/closeout_audit.json"]

    closeout = json.loads(read(closeout_path))
    closeout_summary = closeout.get("result_record_summary")
    if not isinstance(closeout_summary, dict):
        return ["results/closeout_audit.json missing result_record_summary"]

    expected = {
        "total_records": summary.total_rows,
        "non_overlay_records": summary.non_overlay_rows,
        "paper_denominator_records": summary.paper_denominator_rows,
    }
    for key, value in expected.items():
        if closeout_summary.get(key) != value:
            errors.append(
                f"results/closeout_audit.json result_record_summary.{key} is "
                f"{closeout_summary.get(key)!r}, expected {value!r}"
            )
    return errors


def check_denominator_status(ara_root: Path, file_count: int, summary: ResultSummary) -> list[str]:
    path = ara_root / "results/denominator_status.md"
    if not path.is_file():
        return ["missing results/denominator_status.md"]

    errors: list[str] = []
    text = read(path)
    expected_file_count = f"passes with {file_count} files"
    if expected_file_count not in text:
        errors.append(f"results/denominator_status.md must say `{expected_file_count}`")

    if summary.total_rows:
        expected_result_state = (
            f"Current result JSONL state: {summary.jsonl_files} file(s), "
            f"{summary.total_rows} row(s), {summary.overlay_rows} overlay row(s), "
            f"{summary.paper_denominator_rows} paper-denominator row(s)."
        )
        if expected_result_state not in text:
            errors.append("results/denominator_status.md missing current result JSONL state sentence")
    return errors


def check_validation_log(ara_root: Path, summary: ResultSummary) -> list[str]:
    path = ara_root / "validation.md"
    if not path.is_file():
        return ["missing validation.md"]

    errors: list[str] = []
    text = read(path)
    if summary.total_rows > 0 and "No `results/*.jsonl` files exist yet" in text:
        errors.append("validation.md still says no results/*.jsonl files exist")
    if "Status Doc Consistency Check" not in text:
        errors.append("validation.md missing Status Doc Consistency Check section")
    return errors


def check_status_docs(repo_root: Path, ara_root: Path) -> list[str]:
    del repo_root
    errors: list[str] = []
    file_count = ara_file_count(ara_root)
    summary = summarize_results(ara_root)
    errors.extend(check_closeout_summary(ara_root, summary))
    errors.extend(check_denominator_status(ara_root, file_count, summary))
    errors.extend(check_validation_log(ara_root, summary))
    return errors


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("ara_dir", type=Path)
    args = parser.parse_args()
    ara_root = args.ara_dir
    repo_root = repo_root_for(ara_root.resolve())

    errors = check_status_docs(repo_root, ara_root)
    if errors:
        for error in errors:
            print(f"FAIL: {error}")
        return 1
    print(f"PASS: {ara_root} status docs")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
