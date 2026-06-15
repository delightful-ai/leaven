#!/usr/bin/env python3
"""Check the Trace2Skill official-eval importer against a no-spend fixture."""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any


EXPECTED_METRICS = {
    "official_instance_accuracy": 50.0,
    "official_test_case_accuracy": 75.0,
    "official_avg_soft_score": 75.0,
    "official_avg_hard_score": 50.0,
}
APPROVAL_ARTIFACT = "docs/ara/trace2skill_spreadsheetbench/results/full_run_plan.md"
FIXTURE = "scripts/fixtures/trace2skill_eval_official_results_sample.json"


def repo_root_for(ara_root: Path) -> Path:
    for candidate in (ara_root, *ara_root.parents):
        if (candidate / FIXTURE).is_file():
            return candidate
    return Path.cwd()


def importer_base_args(output: Path) -> list[str]:
    return [
        "scripts/import_trace2skill_eval_results.py",
        "--eval-results",
        FIXTURE,
        "--output",
        output.as_posix(),
        "--run-id",
        "trace2skill-import-fixture",
        "--created-at",
        "2026-06-14T00:00:02Z",
        "--proof-classification",
        "paper-subset",
        "--runbook-stage-id",
        "G2",
        "--split",
        "fixture",
        "--case-range",
        "0..2",
        "--case-count",
        "2",
        "--denominator",
        "fixture-subset-not-paper",
        "--model-id",
        "fixture-model",
        "--serving-backend",
        "fixture-backend",
        "--seed",
        "41",
        "--skill-kind",
        "fixture-skill",
        "--approval-artifact-path",
        APPROVAL_ARTIFACT,
        "--source-command",
        "fixture importer smoke",
    ]


def run_importer(repo_root: Path, args: list[str]) -> subprocess.CompletedProcess[str]:
    command = [sys.executable, *args]
    print("$ " + " ".join(command))
    return subprocess.run(
        command,
        cwd=repo_root,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        check=False,
    )


def load_jsonl(path: Path) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    for line in path.read_text(encoding="utf-8").splitlines():
        if line.strip():
            loaded = json.loads(line)
            if not isinstance(loaded, dict):
                raise ValueError(f"{path} contains a non-object JSONL row")
            rows.append(loaded)
    return rows


def expect_failure(
    repo_root: Path,
    args: list[str],
    expected_stderr: str,
    errors: list[str],
    label: str,
) -> None:
    result = run_importer(repo_root, args)
    if result.returncode == 0:
        errors.append(f"{label}: importer unexpectedly succeeded")
        return
    if expected_stderr not in result.stderr:
        errors.append(
            f"{label}: stderr did not contain {expected_stderr!r}; stderr was {result.stderr!r}"
        )


def check_positive_import(repo_root: Path, output: Path, errors: list[str]) -> None:
    result = run_importer(repo_root, importer_base_args(output))
    if result.returncode != 0:
        errors.append(f"positive import failed: {result.stderr.strip()}")
        return

    try:
        rows = load_jsonl(output)
    except (json.JSONDecodeError, ValueError) as exc:
        errors.append(str(exc))
        return

    if len(rows) != 4:
        errors.append(f"positive import wrote {len(rows)} rows, expected 4")
        return

    observed = {row.get("metric_name"): row for row in rows}
    if set(observed) != set(EXPECTED_METRICS):
        errors.append(f"positive import metric names were {sorted(observed)!r}")
        return

    for metric_name, expected_value in EXPECTED_METRICS.items():
        row = observed[metric_name]
        prefix = f"positive import {metric_name}"
        if row.get("proof_classification") != "paper-subset":
            errors.append(f"{prefix}: proof_classification is {row.get('proof_classification')!r}")
        if row.get("metric_value") != expected_value:
            errors.append(f"{prefix}: metric_value is {row.get('metric_value')!r}")
        if row.get("metric_unit") != "percent":
            errors.append(f"{prefix}: metric_unit is {row.get('metric_unit')!r}")
        if row.get("plot_binding") is not None:
            errors.append(f"{prefix}: plot_binding must stay null for fixture rows")

        artifact_paths = row.get("artifact_paths")
        if not isinstance(artifact_paths, list):
            errors.append(f"{prefix}: artifact_paths is not a list")
            continue
        for expected_path in (FIXTURE, APPROVAL_ARTIFACT):
            if expected_path not in artifact_paths:
                errors.append(f"{prefix}: artifact_paths missing {expected_path}")

        extra = row.get("extra")
        if not isinstance(extra, dict):
            errors.append(f"{prefix}: extra is not an object")
            continue
        if extra.get("runbook_stage_id") != "G2":
            errors.append(f"{prefix}: extra.runbook_stage_id is {extra.get('runbook_stage_id')!r}")
        if extra.get("approval_artifact_paths") != [APPROVAL_ARTIFACT]:
            errors.append(f"{prefix}: approval_artifact_paths is {extra.get('approval_artifact_paths')!r}")
        if not isinstance(extra.get("source_metric"), str):
            errors.append(f"{prefix}: extra.source_metric missing")


def check_runbook_stage(repo_root: Path, ara_root: Path, errors: list[str]) -> None:
    runbook = json.loads((ara_root / "results/full_denominator_runbook.json").read_text(encoding="utf-8"))
    stages = {
        stage.get("id"): stage
        for stage in runbook.get("stages", [])
        if isinstance(stage, dict)
    }
    stage = stages.get("G2")
    if stage is None:
        errors.append("runbook stage G2 is missing")
    elif stage.get("allowed_label") != "paper-subset":
        errors.append(f"runbook stage G2 allowed_label is {stage.get('allowed_label')!r}")
    if not (repo_root / APPROVAL_ARTIFACT).is_file():
        errors.append(f"approval artifact is not inspectable: {APPROVAL_ARTIFACT}")


def check_importer_fixture(repo_root: Path, ara_root: Path) -> list[str]:
    errors: list[str] = []
    target_dir = repo_root / "target"
    target_dir.mkdir(exist_ok=True)
    check_runbook_stage(repo_root, ara_root, errors)

    with tempfile.TemporaryDirectory(prefix="trace2skill-importer-fixture-", dir=target_dir) as tmp:
        tmp_path = Path(tmp)
        output = tmp_path / "imported.jsonl"
        check_positive_import(repo_root, output, errors)

        missing_stage_args = importer_base_args(tmp_path / "missing-stage.jsonl")
        stage_flag_index = missing_stage_args.index("--runbook-stage-id")
        del missing_stage_args[stage_flag_index : stage_flag_index + 2]
        expect_failure(
            repo_root,
            missing_stage_args,
            "--runbook-stage-id",
            errors,
            "missing runbook stage",
        )

        missing_approval_args = importer_base_args(tmp_path / "missing-approval.jsonl")
        approval_flag_index = missing_approval_args.index("--approval-artifact-path")
        del missing_approval_args[approval_flag_index : approval_flag_index + 2]
        expect_failure(
            repo_root,
            missing_approval_args,
            "paper-subset requires at least one --approval-artifact-path",
            errors,
            "missing approval artifact",
        )

        paper_denominator_args = importer_base_args(tmp_path / "paper-denominator.jsonl")
        proof_index = paper_denominator_args.index("--proof-classification") + 1
        paper_denominator_args[proof_index] = "paper-denominator-reproduction"
        stage_id_index = paper_denominator_args.index("--runbook-stage-id") + 1
        paper_denominator_args[stage_id_index] = "G9"
        denominator_index = paper_denominator_args.index("--denominator") + 1
        paper_denominator_args[denominator_index] = "SpreadsheetBench-Verified-200-case-paper-denominator"
        case_count_index = paper_denominator_args.index("--case-count") + 1
        paper_denominator_args[case_count_index] = "200"
        expect_failure(
            repo_root,
            paper_denominator_args,
            "refusing paper-denominator-reproduction without --allow-paper-denominator-reproduction",
            errors,
            "missing paper-denominator allow flag",
        )

    return errors


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "ara_dir",
        type=Path,
        default=Path("docs/ara/trace2skill_spreadsheetbench"),
        nargs="?",
    )
    args = parser.parse_args()

    ara_root = args.ara_dir.resolve()
    repo_root = repo_root_for(ara_root)
    errors = check_importer_fixture(repo_root.resolve(), ara_root)
    if errors:
        for error in errors:
            print(f"ERROR: {error}", file=sys.stderr)
        return 1
    print(f"PASS: {args.ara_dir} official-eval importer fixture")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
