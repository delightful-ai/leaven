#!/usr/bin/env python3
"""Check Trace2Skill Leaven result rows before they can feed plots or closeout."""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any


NON_OVERLAY_ONLY_CLASSIFICATIONS = {
    "mechanics-smoke",
    "deterministic-one-case",
    "model-one-case",
    "evolving-split-run",
    "training-validation-candidate",
}
PAPER_DENOMINATOR_CLASSIFICATIONS = {
    "held-out-single-seed-candidate",
    "seed-aggregate-candidate",
    "paper-denominator-candidate",
    "paper-denominator-reproduction",
}
APPROVAL_REQUIRED_PROOF_CLASSIFICATIONS = {
    "model-one-case",
    "paper-subset",
    "evolving-split-run",
    "training-validation-candidate",
    "held-out-single-seed-candidate",
    "seed-aggregate-candidate",
    "paper-denominator-candidate",
    "paper-denominator-reproduction",
}
PLOT_AXIS_UNITS = {
    ("same_model_deepening_vrf", "left"): {"percent"},
    ("avg_improvement", "left"): {"delta_points"},
    ("parallel_vs_sequential", "left"): {"percent"},
    ("parallel_vs_sequential", "right"): {"minutes"},
    ("reasoningbank", "left"): {"percent"},
}


def load_jsonl(path: Path) -> list[tuple[int, dict[str, Any]]]:
    rows: list[tuple[int, dict[str, Any]]] = []
    for line_number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), start=1):
        if not line.strip():
            continue
        record = json.loads(line)
        if not isinstance(record, dict):
            raise ValueError(f"{path}:{line_number} must be a JSON object")
        rows.append((line_number, record))
    return rows


def check_artifact_path(repo_root: Path, prefix: str, rel_path: str, errors: list[str]) -> None:
    if not isinstance(rel_path, str) or not rel_path:
        errors.append(f"{prefix} artifact path entries must be non-empty strings")
        return
    if Path(rel_path).is_absolute():
        errors.append(f"{prefix} artifact path must be repo-relative, got absolute path {rel_path!r}")
        return
    if not (repo_root / rel_path).is_file():
        errors.append(f"{prefix} artifact path is not inspectable: {rel_path}")


def check_record(
    repo_root: Path,
    path: Path,
    line_number: int,
    record: dict[str, Any],
    runbook_stages: dict[str, dict[str, Any]],
    errors: list[str],
) -> None:
    prefix = f"{path.relative_to(repo_root)}:{line_number}"
    proof = record.get("proof_classification")
    binding = record.get("plot_binding")
    denominator = record.get("dataset_slice", {}).get("denominator")
    extra = record.get("extra")

    stage_id = extra.get("runbook_stage_id") if isinstance(extra, dict) else None
    if not isinstance(stage_id, str) or not stage_id:
        errors.append(f"{prefix} extra.runbook_stage_id must name the originating runbook stage")
    else:
        stage = runbook_stages.get(stage_id)
        if stage is None:
            errors.append(f"{prefix} extra.runbook_stage_id {stage_id!r} is not in full_denominator_runbook.json")
        elif stage.get("allowed_label") != proof:
            errors.append(
                f"{prefix} proof_classification {proof!r} does not match runbook stage "
                f"{stage_id} allowed_label {stage.get('allowed_label')!r}"
            )

    artifact_paths = record.get("artifact_paths")
    if not isinstance(artifact_paths, list) or not artifact_paths:
        errors.append(f"{prefix} artifact_paths must be a non-empty list")
    else:
        for rel_path in artifact_paths:
            check_artifact_path(repo_root, prefix, rel_path, errors)

    skill_path = record.get("skill_source", {}).get("path")
    if skill_path is not None:
        check_artifact_path(repo_root, prefix, skill_path, errors)

    if proof in APPROVAL_REQUIRED_PROOF_CLASSIFICATIONS:
        approval_paths = extra.get("approval_artifact_paths") if isinstance(extra, dict) else None
        if not isinstance(approval_paths, list) or not approval_paths:
            errors.append(f"{prefix} {proof} rows must include extra.approval_artifact_paths")
        else:
            for approval_path in approval_paths:
                check_artifact_path(repo_root, prefix, approval_path, errors)
                if isinstance(artifact_paths, list) and approval_path not in artifact_paths:
                    errors.append(
                        f"{prefix} approval artifact {approval_path!r} must also appear in artifact_paths"
                    )

    if proof in NON_OVERLAY_ONLY_CLASSIFICATIONS and binding is not None:
        errors.append(f"{prefix} {proof} rows must keep plot_binding null")

    if binding is None:
        return

    if not isinstance(binding, dict):
        errors.append(f"{prefix} plot_binding must be an object or null")
        return

    panel = binding.get("panel")
    axis = binding.get("axis")
    units = PLOT_AXIS_UNITS.get((panel, axis))
    if units is None:
        errors.append(f"{prefix} plot_binding panel/axis is not overlay-eligible: {panel!r}/{axis!r}")
    elif record.get("metric_unit") not in units:
        errors.append(
            f"{prefix} metric_unit {record.get('metric_unit')!r} does not match {panel!r}/{axis!r} units {sorted(units)!r}"
        )

    if not isinstance(denominator, str) or not denominator:
        errors.append(f"{prefix} overlay rows must have a non-empty dataset_slice.denominator")
    elif "one-case" in denominator:
        errors.append(f"{prefix} one-case denominator rows must not be plotted on paper-target panels")

    if proof == "paper-subset" and "subset" not in str(denominator):
        errors.append(f"{prefix} paper-subset overlays must use an explicit subset denominator")

    if proof in PAPER_DENOMINATOR_CLASSIFICATIONS:
        case_count = record.get("dataset_slice", {}).get("case_count")
        if not isinstance(case_count, int) or case_count < 200:
            errors.append(f"{prefix} paper-denominator rows must cover at least the 200-case paper split")
        if record.get("seed") not in {41, 42, 43, "41", "42", "43"}:
            errors.append(f"{prefix} paper-denominator rows must use seed 41, 42, or 43")

    if proof == "paper-denominator-reproduction":
        if record.get("serving_backend") != "vLLM":
            errors.append(f"{prefix} paper-denominator-reproduction rows must use vLLM")
        if record.get("model_id") not in {"Qwen3.5-122B-A10B", "Qwen3.5-35B-A3B"}:
            errors.append(f"{prefix} paper-denominator-reproduction row has non-paper model_id")


def repo_root_for(ara_root: Path) -> Path:
    for candidate in (ara_root, *ara_root.parents):
        if (candidate / "docs/ara/trace2skill_spreadsheetbench/results").is_dir():
            return candidate
    return Path.cwd()


def check_result_intake(repo_root: Path, ara_root: Path) -> list[str]:
    errors: list[str] = []
    repo_root = repo_root.resolve()
    ara_root = ara_root.resolve()
    results_dir = ara_root / "results"
    runbook = json.loads((ara_root / "results/full_denominator_runbook.json").read_text(encoding="utf-8"))
    runbook_stages = {
        stage["id"]: stage
        for stage in runbook.get("stages", [])
        if isinstance(stage, dict) and isinstance(stage.get("id"), str)
    }
    for path in sorted(results_dir.glob("*.jsonl")):
        try:
            rows = load_jsonl(path)
        except (json.JSONDecodeError, ValueError) as exc:
            errors.append(str(exc))
            continue
        for line_number, record in rows:
            check_record(repo_root, path, line_number, record, runbook_stages, errors)
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
    errors = check_result_intake(repo_root, ara_root)
    if errors:
        for error in errors:
            print(f"ERROR: {error}", file=sys.stderr)
        return 1
    print(f"PASS: {args.ara_dir} result intake")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
