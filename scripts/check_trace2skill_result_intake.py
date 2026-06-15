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


def parse_case_range(raw: Any) -> tuple[int, int] | None:
    if not isinstance(raw, str):
        return None
    match = re.fullmatch(r"(\d+)\.\.(\d+)", raw)
    if match is None:
        return None
    start = int(match.group(1))
    end = int(match.group(2))
    if end < start:
        return None
    return start, end


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
    stage: dict[str, Any] | None,
    dataset_slice: Any,
    errors: list[str],
) -> None:
    if stage is None:
        return
    stage_id = stage.get("id")
    if not isinstance(dataset_slice, dict):
        errors.append(f"{prefix} dataset_slice must be an object")
        return

    expected = stage.get("expected_dataset_slice")
    if expected is None:
        return
    if not isinstance(expected, dict):
        errors.append(f"{prefix} runbook stage {stage_id!r} expected_dataset_slice must be an object or null")
        return

    kind = expected.get("kind")
    denominator = dataset_slice.get("denominator")
    case_range = dataset_slice.get("case_range")
    case_count = dataset_slice.get("case_count")

    if kind == "one-case":
        if case_range != expected.get("case_range"):
            errors.append(f"{prefix} {stage_id} rows must use dataset_slice.case_range {expected.get('case_range')!r}")
        if case_count != expected.get("case_count"):
            errors.append(f"{prefix} {stage_id} rows must use dataset_slice.case_count {expected.get('case_count')}")
        required = expected.get("denominator_contains")
        if not isinstance(denominator, str) or not isinstance(required, str) or required not in denominator:
            errors.append(f"{prefix} {stage_id} rows must name a denominator containing {required!r}")
        return

    if kind == "held-out-subset":
        parsed = parse_case_range(case_range)
        if parsed is None:
            errors.append(f"{prefix} {stage_id} held-out subset rows must use a numeric dataset_slice.case_range")
        else:
            start, end = parsed
            if start < expected.get("range_start_min"):
                errors.append(f"{prefix} {stage_id} held-out subset rows must start at or after {expected.get('range_start_min')}")
            if end > expected.get("range_end_max"):
                errors.append(f"{prefix} {stage_id} held-out subset rows must end at or before {expected.get('range_end_max')}")
            if isinstance(case_count, int) and case_count != end - start:
                errors.append(f"{prefix} {stage_id} held-out subset rows must have case_count equal to case_range length")
        if (
            not isinstance(case_count, int)
            or case_count < expected.get("case_count_min")
            or case_count >= expected.get("case_count_max_exclusive")
        ):
            errors.append(f"{prefix} {stage_id} paper-subset rows must stay below the 200-case paper denominator")
        if not isinstance(denominator, str) or "subset" not in denominator:
            errors.append(f"{prefix} {stage_id} paper-subset rows must use an explicit subset denominator")
        for forbidden in expected.get("forbidden_denominator_fragments", []):
            if isinstance(forbidden, str) and isinstance(denominator, str) and forbidden in denominator:
                errors.append(f"{prefix} {stage_id} paper-subset rows must not name a full paper denominator")
        return

    if kind == "exact-range":
        if case_range != expected.get("case_range"):
            errors.append(f"{prefix} {stage_id} rows must use dataset_slice.case_range {expected.get('case_range')!r}")
        if case_count != expected.get("case_count"):
            errors.append(f"{prefix} {stage_id} rows must use dataset_slice.case_count {expected.get('case_count')}")
        if denominator != expected.get("denominator"):
            errors.append(f"{prefix} {stage_id} rows must use dataset_slice.denominator {expected.get('denominator')!r}")
        return

    if kind in {"aggregate", "full-paper"} and denominator != expected.get("denominator"):
        errors.append(f"{prefix} {stage_id} rows must use dataset_slice.denominator {expected.get('denominator')!r}")


def normalize_seed(value: Any) -> int | None:
    if isinstance(value, int) and not isinstance(value, bool):
        return value
    if isinstance(value, str):
        try:
            return int(value)
        except ValueError:
            return None
    return None


def normalize_seed_list(value: Any) -> list[int] | None:
    if not isinstance(value, list):
        return None
    normalized: list[int] = []
    for item in value:
        seed = normalize_seed(item)
        if seed is None:
            return None
        normalized.append(seed)
    return normalized


def check_stage_seed_policy(
    prefix: str,
    stage: dict[str, Any] | None,
    record: dict[str, Any],
    errors: list[str],
) -> None:
    if stage is None:
        return
    stage_id = stage.get("id")
    expected = stage.get("expected_seed_policy")
    if expected is None:
        return
    if not isinstance(expected, dict):
        errors.append(f"{prefix} runbook stage {stage_id!r} expected_seed_policy must be an object or null")
        return

    kind = expected.get("kind")
    seed = normalize_seed(record.get("seed"))
    expected_seeds = normalize_seed_list(expected.get("seeds"))

    if kind == "exact":
        expected_seed = normalize_seed(expected.get("seed"))
        if seed != expected_seed:
            errors.append(f"{prefix} {stage_id} rows must use seed {expected_seed}")
        return

    if kind == "one-of":
        if expected_seeds is None or seed not in expected_seeds:
            errors.append(f"{prefix} {stage_id} rows must use one of seeds {expected.get('seeds')!r}")
        return

    if kind == "all-of":
        extra = record.get("extra")
        observed = normalize_seed_list(extra.get("seeds") if isinstance(extra, dict) else None)
        if expected_seeds is None or observed != expected_seeds:
            errors.append(f"{prefix} {stage_id} aggregate rows must carry extra.seeds {expected.get('seeds')!r}")
        return

    errors.append(f"{prefix} runbook stage {stage_id!r} expected_seed_policy has unknown kind {kind!r}")


def check_stage_runtime_policy(
    prefix: str,
    stage: dict[str, Any] | None,
    record: dict[str, Any],
    errors: list[str],
) -> None:
    if stage is None:
        return
    stage_id = stage.get("id")
    expected = stage.get("expected_runtime_policy")
    if expected is None:
        return
    if not isinstance(expected, dict):
        errors.append(f"{prefix} runbook stage {stage_id!r} expected_runtime_policy must be an object or null")
        return

    runtime = record.get("runtime")
    if not isinstance(runtime, dict):
        errors.append(f"{prefix} runtime must be an object")
        return

    kind = expected.get("kind")
    if kind not in {"upstream-run", "skill-evolution"}:
        errors.append(f"{prefix} runbook stage {stage_id!r} expected_runtime_policy has unknown kind {kind!r}")
        return

    if runtime.get("workers") != expected.get("workers"):
        errors.append(f"{prefix} {stage_id} rows must use runtime.workers {expected.get('workers')}")
    if runtime.get("max_turns") != expected.get("max_turns"):
        errors.append(f"{prefix} {stage_id} rows must use runtime.max_turns {expected.get('max_turns')}")

    if kind == "skill-evolution":
        extra = record.get("extra")
        merge_batch_size = extra.get("merge_batch_size") if isinstance(extra, dict) else None
        if merge_batch_size != expected.get("merge_batch_size"):
            errors.append(
                f"{prefix} {stage_id} rows must use extra.merge_batch_size {expected.get('merge_batch_size')}"
            )


def check_stage_command_policy(
    prefix: str,
    stage: dict[str, Any] | None,
    record: dict[str, Any],
    errors: list[str],
) -> None:
    if stage is None:
        return
    stage_id = stage.get("id")
    expected = stage.get("expected_command_policy")
    if expected is None:
        return
    if not isinstance(expected, dict):
        errors.append(f"{prefix} runbook stage {stage_id!r} expected_command_policy must be an object or null")
        return

    extra = record.get("extra")
    command_policy = extra.get("command_policy") if isinstance(extra, dict) else None
    expected_kind = expected.get("kind")
    if command_policy != expected_kind:
        errors.append(f"{prefix} {stage_id} rows must use extra.command_policy {expected_kind!r}")

    source_command = record.get("source_command")
    if not isinstance(source_command, str) or not source_command:
        errors.append(f"{prefix} source_command must be a non-empty string")
        return
    fragments = expected.get("required_source_command_fragments")
    if not isinstance(fragments, list) or not all(isinstance(item, str) and item for item in fragments):
        errors.append(f"{prefix} runbook stage {stage_id!r} expected_command_policy fragments must be strings")
        return
    for fragment in fragments:
        if fragment not in source_command:
            errors.append(f"{prefix} {stage_id} source_command must include {fragment!r}")


def source_result_paths(extra: Any) -> list[str] | None:
    if not isinstance(extra, dict):
        return None
    paths = extra.get("source_result_paths")
    if not isinstance(paths, list) or not all(isinstance(path, str) and path for path in paths):
        return None
    return paths


def source_rows(
    repo_root: Path,
    rel_paths: list[str],
    errors: list[str],
    prefix: str,
) -> list[tuple[Path, int, dict[str, Any]]]:
    rows: list[tuple[Path, int, dict[str, Any]]] = []
    for rel_path in rel_paths:
        path = repo_root / rel_path
        try:
            loaded = load_jsonl(path)
        except (json.JSONDecodeError, OSError, ValueError) as exc:
            errors.append(f"{prefix} source result path {rel_path!r} is not valid JSONL: {exc}")
            continue
        rows.extend((path, line_number, record) for line_number, record in loaded)
    return rows


def check_stage_aggregate_policy(
    repo_root: Path,
    current_path: Path,
    line_number: int,
    prefix: str,
    stage: dict[str, Any] | None,
    record: dict[str, Any],
    runbook_stages: dict[str, dict[str, Any]],
    artifact_paths: Any,
    errors: list[str],
    aggregate_stack: set[str],
) -> None:
    if stage is None:
        return
    stage_id = stage.get("id")
    expected = stage.get("expected_aggregate_policy")
    if expected is None:
        return
    if not isinstance(expected, dict):
        errors.append(f"{prefix} runbook stage {stage_id!r} expected_aggregate_policy must be an object or null")
        return

    extra = record.get("extra")
    paths = source_result_paths(extra)
    minimum = expected.get("source_result_paths_min")
    if paths is None:
        errors.append(f"{prefix} {stage_id} aggregate rows must carry extra.source_result_paths")
        return
    if isinstance(minimum, int) and len(set(paths)) < minimum:
        errors.append(f"{prefix} {stage_id} aggregate rows must cite at least {minimum} source result path(s)")

    current_rel = current_path.relative_to(repo_root).as_posix()
    current_key = f"{current_rel}:{line_number}"
    if current_key in aggregate_stack:
        errors.append(f"{prefix} aggregate source graph contains a cycle at {current_key}")
        return
    next_stack = {*aggregate_stack, current_key}
    for rel_path in paths:
        if rel_path == current_rel:
            errors.append(f"{prefix} {stage_id} aggregate rows must not cite their own result file as source")
        check_artifact_path(repo_root, prefix, rel_path, errors)
        if isinstance(artifact_paths, list) and rel_path not in artifact_paths:
            errors.append(f"{prefix} aggregate source result {rel_path!r} must also appear in artifact_paths")

    rows = source_rows(repo_root, paths, errors, prefix)
    kind = expected.get("kind")
    if kind == "seed-aggregate":
        required_seeds = normalize_seed_list(expected.get("required_seeds"))
        if required_seeds is None:
            errors.append(f"{prefix} runbook stage {stage_id!r} expected_aggregate_policy required_seeds must be a seed list")
            return
        source_stage_id = expected.get("source_runbook_stage_id")
        source_proof = expected.get("source_proof_classification")
        observed: set[int] = set()
        for source_path, line_number, source in rows:
            source_errors: list[str] = []
            check_record(
                repo_root,
                source_path,
                line_number,
                source,
                runbook_stages,
                source_errors,
                validate_aggregate=False,
                aggregate_stack=next_stack,
            )
            if source_errors:
                errors.append(
                    f"{prefix} {stage_id} source row {source_path.relative_to(repo_root)}:{line_number} "
                    f"does not pass result intake: {source_errors!r}"
                )
            source_extra = source.get("extra")
            if not isinstance(source_extra, dict):
                continue
            if source_extra.get("runbook_stage_id") != source_stage_id:
                continue
            if source.get("proof_classification") != source_proof:
                continue
            seed = normalize_seed(source.get("seed"))
            if seed is not None:
                observed.add(seed)
        missing = [seed for seed in required_seeds if seed not in observed]
        if missing:
            errors.append(f"{prefix} {stage_id} aggregate rows must cite source result rows for seeds {missing!r}")
        return

    if kind == "full-paper":
        classifications = expected.get("source_proof_classifications")
        if not isinstance(classifications, list) or not all(isinstance(item, str) for item in classifications):
            errors.append(f"{prefix} runbook stage {stage_id!r} expected_aggregate_policy source_proof_classifications must be strings")
            return
        matching_sources = [
            (source_path, source_line_number, source)
            for source_path, source_line_number, source in rows
            if source.get("proof_classification") in classifications
        ]
        if not matching_sources:
            errors.append(f"{prefix} {stage_id} full-paper rows must cite aggregate or candidate source result rows")
            return
        for source_path, source_line_number, source in matching_sources:
            source_errors: list[str] = []
            check_record(
                repo_root,
                source_path,
                source_line_number,
                source,
                runbook_stages,
                source_errors,
                validate_aggregate=True,
                aggregate_stack=next_stack,
            )
            if source_errors:
                errors.append(
                    f"{prefix} {stage_id} source row {source_path.relative_to(repo_root)}:{source_line_number} "
                    f"does not pass result intake: {source_errors!r}"
                )
        return

    errors.append(f"{prefix} runbook stage {stage_id!r} expected_aggregate_policy has unknown kind {kind!r}")


def check_record(
    repo_root: Path,
    path: Path,
    line_number: int,
    record: dict[str, Any],
    runbook_stages: dict[str, dict[str, Any]],
    errors: list[str],
    validate_aggregate: bool = True,
    aggregate_stack: set[str] | None = None,
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
    check_stage_dataset_slice(prefix, stage, dataset_slice, errors)
    check_stage_seed_policy(prefix, stage, record, errors)
    check_stage_runtime_policy(prefix, stage, record, errors)
    check_stage_command_policy(prefix, stage, record, errors)

    artifact_paths = record.get("artifact_paths")
    if not isinstance(artifact_paths, list) or not artifact_paths:
        errors.append(f"{prefix} artifact_paths must be a non-empty list")
    else:
        for rel_path in artifact_paths:
            check_artifact_path(repo_root, prefix, rel_path, errors)
    if validate_aggregate:
        check_stage_aggregate_policy(
            repo_root,
            path,
            line_number,
            prefix,
            stage,
            record,
            runbook_stages,
            artifact_paths,
            errors,
            aggregate_stack or set(),
        )
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
