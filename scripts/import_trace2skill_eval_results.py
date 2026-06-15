#!/usr/bin/env python3
"""Import official Trace2Skill SpreadsheetBench evaluation JSON into Leaven result rows."""

from __future__ import annotations

import argparse
import importlib.util
import json
import math
import sys
from pathlib import Path
from typing import Any


RESULT_SCHEMA_VERSION = "leaven.trace2skill.result.v1"
ALLOWED_PROOF_CLASSIFICATIONS = {
    "mechanics-smoke",
    "deterministic-one-case",
    "model-one-case",
    "paper-subset",
    "evolving-split-run",
    "training-validation-candidate",
    "held-out-single-seed-candidate",
    "seed-aggregate-candidate",
    "paper-denominator-candidate",
    "paper-denominator-reproduction",
}
OVERALL_METRICS = {
    "instance_accuracy": "official_instance_accuracy",
    "test_case_accuracy": "official_test_case_accuracy",
    "avg_soft_score": "official_avg_soft_score",
    "avg_hard_score": "official_avg_hard_score",
}
PLOT_CAPABLE_PROOF_CLASSIFICATIONS = {
    "paper-subset",
    "held-out-single-seed-candidate",
    "seed-aggregate-candidate",
    "paper-denominator-candidate",
    "paper-denominator-reproduction",
}
APPROVAL_REQUIRED_PROOF_CLASSIFICATIONS = ALLOWED_PROOF_CLASSIFICATIONS - {
    "mechanics-smoke",
    "deterministic-one-case",
}
SUPPORTED_RESULT_PANELS = {
    "same_model_deepening_vrf",
    "avg_improvement",
    "parallel_vs_sequential",
    "reasoningbank",
}


def load_json(path: Path) -> dict[str, Any]:
    require_existing_path(path, "--eval-results")
    loaded = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(loaded, dict):
        raise ValueError(f"{path} must contain a JSON object")
    return loaded


def require_existing_path(path: Path, label: str) -> None:
    if not path.exists():
        raise FileNotFoundError(f"{label} is not inspectable: {path}")


def require_existing_artifact_paths(paths: list[str], label: str) -> None:
    for raw in paths:
        require_existing_path(Path(raw), label)


def require_number(mapping: dict[str, Any], field: str, prefix: str) -> float:
    value = mapping.get(field)
    if not isinstance(value, (int, float)) or isinstance(value, bool):
        raise ValueError(f"{prefix}.{field} must be numeric")
    return float(value)


def require_int(mapping: dict[str, Any], field: str, prefix: str) -> int:
    value = mapping.get(field)
    if not isinstance(value, int) or isinstance(value, bool):
        raise ValueError(f"{prefix}.{field} must be an integer")
    return value


def close_enough(left: float, right: float) -> bool:
    return math.isclose(left, right, rel_tol=0, abs_tol=1e-9)


def validate_eval_result(eval_result: dict[str, Any], expected_case_count: int) -> dict[str, Any]:
    summary = eval_result.get("summary")
    results = eval_result.get("results")
    if not isinstance(summary, dict):
        raise ValueError("eval result missing summary object")
    if not isinstance(results, list):
        raise ValueError("eval result missing results array")

    total_instances = require_int(summary, "total_instances", "summary")
    fully_correct = require_int(summary, "fully_correct_instances", "summary")
    total_test_cases = require_int(summary, "total_test_cases", "summary")
    passed_test_cases = require_int(summary, "passed_test_cases", "summary")

    if total_instances != expected_case_count:
        raise ValueError(
            f"summary.total_instances {total_instances} does not match --case-count {expected_case_count}"
        )
    if len(results) != total_instances:
        raise ValueError(f"results length {len(results)} does not match summary.total_instances {total_instances}")
    if not 0 <= fully_correct <= total_instances:
        raise ValueError("summary.fully_correct_instances is outside 0..total_instances")
    if total_test_cases < 1:
        raise ValueError("summary.total_test_cases must be positive")
    if not 0 <= passed_test_cases <= total_test_cases:
        raise ValueError("summary.passed_test_cases is outside 0..total_test_cases")

    instance_accuracy = require_number(summary, "instance_accuracy", "summary")
    test_case_accuracy = require_number(summary, "test_case_accuracy", "summary")
    avg_soft_score = require_number(summary, "avg_soft_score", "summary")
    avg_hard_score = require_number(summary, "avg_hard_score", "summary")
    for field, value in (
        ("instance_accuracy", instance_accuracy),
        ("test_case_accuracy", test_case_accuracy),
        ("avg_soft_score", avg_soft_score),
        ("avg_hard_score", avg_hard_score),
    ):
        if not 0 <= value <= 1:
            raise ValueError(f"summary.{field} must be in 0..1")

    if not close_enough(instance_accuracy, fully_correct / total_instances):
        raise ValueError("summary.instance_accuracy does not match fully_correct_instances / total_instances")
    if not close_enough(test_case_accuracy, passed_test_cases / total_test_cases):
        raise ValueError("summary.test_case_accuracy does not match passed_test_cases / total_test_cases")

    soft_scores: list[float] = []
    hard_scores: list[float] = []
    for index, result in enumerate(results, start=1):
        if not isinstance(result, dict):
            raise ValueError(f"results[{index}] must be an object")
        soft = require_number(result, "soft_score", f"results[{index}]")
        hard = require_number(result, "hard_score", f"results[{index}]")
        if not 0 <= soft <= 1:
            raise ValueError(f"results[{index}].soft_score must be in 0..1")
        if hard not in {0.0, 1.0}:
            raise ValueError(f"results[{index}].hard_score must be 0 or 1")
        soft_scores.append(soft)
        hard_scores.append(hard)

    if not close_enough(avg_soft_score, sum(soft_scores) / len(soft_scores)):
        raise ValueError("summary.avg_soft_score does not match result mean")
    if not close_enough(avg_hard_score, sum(hard_scores) / len(hard_scores)):
        raise ValueError("summary.avg_hard_score does not match result mean")

    by_instruction_type = summary.get("by_instruction_type")
    if not isinstance(by_instruction_type, dict):
        raise ValueError("summary.by_instruction_type must be an object")

    return summary


def parse_seed(raw: str | None) -> int | str | None:
    if raw is None or raw == "" or raw.lower() == "null":
        return None
    try:
        return int(raw)
    except ValueError:
        return raw


def parse_plot_bindings(raw_bindings: list[str], proof_classification: str) -> dict[str, dict[str, str]]:
    parsed: dict[str, dict[str, str]] = {}
    for raw in raw_bindings:
        loaded = json.loads(raw)
        if not isinstance(loaded, dict):
            raise ValueError("--plot-binding-json entries must be JSON objects")
        metric = loaded.get("source_metric")
        if metric not in OVERALL_METRICS:
            raise ValueError(f"plot binding source_metric must be one of {sorted(OVERALL_METRICS)}")
        for field in ("panel", "x_label", "series", "axis"):
            if not isinstance(loaded.get(field), str) or not loaded[field]:
                raise ValueError(f"plot binding for {metric} missing non-empty {field}")
        if loaded["panel"] not in SUPPORTED_RESULT_PANELS:
            raise ValueError(f"plot binding panel {loaded['panel']!r} is unsupported")
        if loaded["axis"] not in {"left", "right"}:
            raise ValueError("plot binding axis must be left or right")
        metric_name = loaded.get("metric_name")
        if metric_name is not None and (not isinstance(metric_name, str) or not metric_name.strip()):
            raise ValueError("plot binding metric_name must be a non-empty string when provided")
        if proof_classification not in PLOT_CAPABLE_PROOF_CLASSIFICATIONS:
            raise ValueError(
                f"{proof_classification} rows cannot carry paper plot bindings; use plot_binding null"
            )
        parsed[metric] = loaded
    return parsed


def percent(value: float) -> float:
    return value * 100.0


def repo_root_for(ara_root: Path) -> Path:
    for candidate in (ara_root, *ara_root.parents):
        if (candidate / "docs/ara/trace2skill_spreadsheetbench/results").is_dir():
            return candidate
    return Path.cwd()


def import_result_intake_checker(repo_root: Path) -> Any:
    path = repo_root / "scripts/check_trace2skill_result_intake.py"
    spec = importlib.util.spec_from_file_location("check_trace2skill_result_intake", path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot import {path}")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def load_runbook_stages(ara_root: Path) -> dict[str, dict[str, Any]]:
    runbook_path = ara_root / "results/full_denominator_runbook.json"
    require_existing_path(runbook_path, "--ara-dir runbook")
    runbook = json.loads(runbook_path.read_text(encoding="utf-8"))
    if not isinstance(runbook, dict):
        raise ValueError(f"{runbook_path} must contain a JSON object")
    return {
        stage["id"]: stage
        for stage in runbook.get("stages", [])
        if isinstance(stage, dict) and isinstance(stage.get("id"), str)
    }


def validate_records_against_result_intake(args: argparse.Namespace, records: list[dict[str, Any]]) -> None:
    ara_root = args.ara_dir.resolve()
    repo_root = repo_root_for(ara_root).resolve()
    output_path = (Path.cwd() / args.output).resolve() if not args.output.is_absolute() else args.output.resolve()
    try:
        output_path.relative_to(repo_root)
    except ValueError as exc:
        raise ValueError(f"--output must be inside the repo for result-intake preflight: {output_path}") from exc

    checker = import_result_intake_checker(repo_root)
    runbook_stages = load_runbook_stages(ara_root)
    errors: list[str] = []
    for line_number, record in enumerate(records, start=1):
        checker.check_record(repo_root, output_path, line_number, record, runbook_stages, errors)
    if errors:
        joined = "\n- ".join(errors)
        raise ValueError(f"result intake preflight failed:\n- {joined}")


def build_records(args: argparse.Namespace) -> list[dict[str, Any]]:
    if args.proof_classification == "paper-denominator-reproduction" and not args.allow_paper_denominator_reproduction:
        raise ValueError(
            "refusing paper-denominator-reproduction without --allow-paper-denominator-reproduction"
        )
    if args.proof_classification in APPROVAL_REQUIRED_PROOF_CLASSIFICATIONS and not args.approval_artifact_path:
        raise ValueError(f"{args.proof_classification} requires at least one --approval-artifact-path")

    eval_result = load_json(args.eval_results)
    summary = validate_eval_result(eval_result, args.case_count)
    seed = parse_seed(args.seed)
    plot_bindings = parse_plot_bindings(args.plot_binding_json, args.proof_classification)
    require_existing_artifact_paths(args.artifact_path, "--artifact-path")
    require_existing_artifact_paths(args.approval_artifact_path, "--approval-artifact-path")

    artifact_paths = [args.eval_results.as_posix(), *args.artifact_path, *args.approval_artifact_path]
    skill_source = {"kind": args.skill_kind}
    if args.skill_path:
        require_existing_path(Path(args.skill_path), "--skill-path")
        skill_source["path"] = args.skill_path

    base = {
        "schema_version": RESULT_SCHEMA_VERSION,
        "run_id": args.run_id,
        "created_at": args.created_at,
        "proof_classification": args.proof_classification,
        "dataset_slice": {
            "name": args.dataset_name,
            "split": args.split,
            "case_range": args.case_range,
            "case_count": args.case_count,
            "denominator": args.denominator,
        },
        "model_id": args.model_id,
        "serving_backend": args.serving_backend,
        "seed": seed,
        "skill_source": skill_source,
        "metric_unit": "percent",
        "cost": {
            "usd": args.cost_usd,
            "prompt_tokens": args.prompt_tokens,
            "completion_tokens": args.completion_tokens,
        },
        "runtime": {
            "seconds": args.runtime_seconds,
            "workers": args.workers,
        },
        "source_command": args.source_command,
        "artifact_paths": artifact_paths,
        "notes": args.notes,
    }

    records = []
    for source_metric, default_metric_name in OVERALL_METRICS.items():
        binding = plot_bindings.get(source_metric)
        record = {
            **base,
            "metric_name": binding.get("metric_name", default_metric_name) if binding else default_metric_name,
            "metric_value": percent(float(summary[source_metric])),
            "plot_binding": {
                "panel": binding["panel"],
                "x_label": binding["x_label"],
                "series": binding["series"],
                "axis": binding["axis"],
            }
            if binding
            else None,
            "extra": {
                "source_metric": source_metric,
                "total_instances": summary["total_instances"],
                "fully_correct_instances": summary["fully_correct_instances"],
                "total_test_cases": summary["total_test_cases"],
                "passed_test_cases": summary["passed_test_cases"],
            },
        }
        if args.proof_classification in APPROVAL_REQUIRED_PROOF_CLASSIFICATIONS:
            record["extra"]["approval_artifact_paths"] = args.approval_artifact_path
        record["extra"]["runbook_stage_id"] = args.runbook_stage_id
        if args.proof_classification == "paper-denominator-reproduction":
            record["extra"]["paper_denominator_reproduction_allowed_by"] = (
                "--allow-paper-denominator-reproduction"
            )
        records.append(record)

    return records


def non_empty(value: str) -> str:
    if not value.strip():
        raise argparse.ArgumentTypeError("value must be non-empty")
    return value


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Import official SpreadsheetBench eval_official_results.json into Leaven JSONL rows."
    )
    parser.add_argument("--eval-results", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument(
        "--ara-dir",
        type=Path,
        default=Path("docs/ara/trace2skill_spreadsheetbench"),
        help="ARA root containing results/full_denominator_runbook.json for pre-write result-intake checks.",
    )
    parser.add_argument("--run-id", type=non_empty, required=True)
    parser.add_argument("--created-at", type=non_empty, required=True)
    parser.add_argument("--proof-classification", choices=sorted(ALLOWED_PROOF_CLASSIFICATIONS), required=True)
    parser.add_argument(
        "--runbook-stage-id",
        type=non_empty,
        required=True,
        help="Full-denominator runbook stage id whose allowed_label matches the proof classification.",
    )
    parser.add_argument("--dataset-name", default="SpreadsheetBench-Verified")
    parser.add_argument("--split", type=non_empty, required=True)
    parser.add_argument("--case-range", type=non_empty, required=True)
    parser.add_argument("--case-count", type=int, required=True)
    parser.add_argument("--denominator", type=non_empty, required=True)
    parser.add_argument("--model-id", type=non_empty, required=True)
    parser.add_argument("--serving-backend", type=non_empty, required=True)
    parser.add_argument("--seed")
    parser.add_argument("--skill-kind", type=non_empty, required=True)
    parser.add_argument("--skill-path")
    parser.add_argument("--source-command", type=non_empty, required=True)
    parser.add_argument("--artifact-path", action="append", default=[])
    parser.add_argument(
        "--approval-artifact-path",
        action="append",
        default=[],
        help="Required for approval-gated rows; names approval/audit evidence.",
    )
    parser.add_argument("--cost-usd", type=float)
    parser.add_argument("--prompt-tokens", type=int)
    parser.add_argument("--completion-tokens", type=int)
    parser.add_argument("--runtime-seconds", type=float)
    parser.add_argument("--workers", type=int)
    parser.add_argument("--notes", default="")
    parser.add_argument(
        "--plot-binding-json",
        action="append",
        default=[],
        help="JSON object with source_metric, panel, x_label, series, axis, and optional metric_name.",
    )
    parser.add_argument(
        "--allow-paper-denominator-reproduction",
        action="store_true",
        help="Explicit guard to prevent accidental full-paper reproduction labels.",
    )
    args = parser.parse_args()

    if args.case_count < 1:
        raise ValueError("--case-count must be positive")

    records = build_records(args)
    validate_records_against_result_intake(args, records)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(
        "".join(json.dumps(record, sort_keys=True) + "\n" for record in records),
        encoding="utf-8",
    )
    print(f"wrote {len(records)} row(s) to {args.output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
