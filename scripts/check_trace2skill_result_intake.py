#!/usr/bin/env python3
"""Check Trace2Skill Leaven result rows before they can feed plots or closeout."""

from __future__ import annotations

import argparse
import json
import re
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
PROMPT_ARTIFACT_WORDS = {
    "prompt",
    "rendered_prompts",
    "stage2_analyst_prompts",
    "stage2_fanout",
    "stage3_merge_prompts",
    "stage3_merge_manifest",
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


def stage_prompt_artifact_patterns(stage: dict[str, Any]) -> list[tuple[str, re.Pattern[str] | None]]:
    expected = stage.get("expected_artifacts")
    if not isinstance(expected, list):
        return []
    patterns: list[tuple[str, re.Pattern[str] | None]] = []
    for artifact in expected:
        artifact_text = str(artifact)
        if not any(word in artifact_text for word in PROMPT_ARTIFACT_WORDS):
            continue
        if "/" not in artifact_text and "." not in artifact_text:
            patterns.append((artifact_text, None))
            continue
        pattern_text = re.escape(artifact_text)
        pattern_text = re.sub(r"\\\{[^}]+\\\}", r"[^/]+", pattern_text)
        pattern_text = re.sub(r"<[^>]+>", r"[^/]+", pattern_text)
        patterns.append((artifact_text, re.compile(pattern_text)))
    return patterns


def check_required_prompt_artifacts(
    prefix: str,
    proof: Any,
    stage: dict[str, Any] | None,
    artifact_paths: Any,
    errors: list[str],
) -> None:
    if proof not in APPROVAL_REQUIRED_PROOF_CLASSIFICATIONS or stage is None:
        return
    if not isinstance(artifact_paths, list):
        return
    artifact_text = "\n".join(str(path) for path in artifact_paths)
    for source, pattern in stage_prompt_artifact_patterns(stage):
        if pattern is None:
            if "prompt" not in artifact_text:
                errors.append(f"{prefix} missing prompt artifact evidence for runbook expectation {source!r}")
            continue
        if not any(pattern.search(str(path)) for path in artifact_paths):
            errors.append(f"{prefix} missing prompt artifact matching runbook expectation {source!r}")


def check_stage_dataset_slice(
    prefix: str,
    stage_id: str | None,
    dataset_slice: Any,
    errors: list[str],
) -> None:
    if not isinstance(stage_id, str):
        return
    if not isinstance(dataset_slice, dict):
        errors.append(f"{prefix} dataset_slice must be an object")
        return

    denominator = dataset_slice.get("denominator")
    case_range = dataset_slice.get("case_range")
    case_count = dataset_slice.get("case_count")

    if stage_id in {"G1", "G1M"}:
        if case_count != 1:
            errors.append(f"{prefix} {stage_id} one-case rows must have dataset_slice.case_count 1")
        if not isinstance(denominator, str) or "one-case-13-1" not in denominator:
            errors.append(f"{prefix} {stage_id} one-case rows must name a one-case-13-1 denominator")
        return

    if stage_id == "G2":
        if not isinstance(case_count, int) or case_count < 1 or case_count >= 200:
            errors.append(f"{prefix} G2 paper-subset rows must stay below the 200-case paper denominator")
        if not isinstance(denominator, str) or "subset" not in denominator:
            errors.append(f"{prefix} G2 paper-subset rows must use an explicit subset denominator")
        if isinstance(denominator, str) and ("paper-denominator" in denominator or "full-paper" in denominator):
            errors.append(f"{prefix} G2 paper-subset rows must not name a full paper denominator")
        return

    exact_stage_slices = {
        "G3": ("0..200", 200, "evolving-split-0..200"),
        "G3V": ("0..200", 200, "training-validation-0..200"),
        "G4": ("200..400", 200, "held-out-200..400"),
    }
    if stage_id in exact_stage_slices:
        expected_range, expected_count, expected_denominator = exact_stage_slices[stage_id]
        if case_range != expected_range:
            errors.append(f"{prefix} {stage_id} rows must use dataset_slice.case_range {expected_range!r}")
        if case_count != expected_count:
            errors.append(f"{prefix} {stage_id} rows must use dataset_slice.case_count {expected_count}")
        if denominator != expected_denominator:
            errors.append(f"{prefix} {stage_id} rows must use dataset_slice.denominator {expected_denominator!r}")
        return

    exact_denominators = {
        "G5": "seed-aggregate-41-42-43",
        "G6": "full-paper-denominator",
    }
    if stage_id in exact_denominators and denominator != exact_denominators[stage_id]:
        errors.append(f"{prefix} {stage_id} rows must use dataset_slice.denominator {exact_denominators[stage_id]!r}")


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
    dataset_slice = record.get("dataset_slice")
    denominator = dataset_slice.get("denominator") if isinstance(dataset_slice, dict) else None
    extra = record.get("extra")

    stage_id = extra.get("runbook_stage_id") if isinstance(extra, dict) else None
    stage: dict[str, Any] | None = None
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
    check_stage_dataset_slice(prefix, stage_id, dataset_slice, errors)

    artifact_paths = record.get("artifact_paths")
    if not isinstance(artifact_paths, list) or not artifact_paths:
        errors.append(f"{prefix} artifact_paths must be a non-empty list")
    else:
        for rel_path in artifact_paths:
            check_artifact_path(repo_root, prefix, rel_path, errors)
    check_required_prompt_artifacts(prefix, proof, stage, artifact_paths, errors)

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
