#!/usr/bin/env python3
"""Check Trace2Skill Leaven result rows before they can feed plots or closeout."""

from __future__ import annotations

import argparse
import importlib.util
import json
import math
import re
import sys
from pathlib import Path
from typing import Any


RESULT_SCHEMA_VERSION = "leaven.trace2skill.result.v1"
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
ALLOWED_PROOF_CLASSIFICATIONS = (
    NON_OVERLAY_ONLY_CLASSIFICATIONS
    | PAPER_DENOMINATOR_CLASSIFICATIONS
    | APPROVAL_REQUIRED_PROOF_CLASSIFICATIONS
)
ALLOWED_METRIC_UNITS = {"percent", "delta_points", "minutes", "fraction"}
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
OPTIONAL_ROW_ARTIFACT_FRAGMENTS = {
    "leaven_results.jsonl",
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


def import_approval_packet_checker() -> Any:
    path = Path(__file__).with_name("check_trace2skill_approval_packet.py")
    spec = importlib.util.spec_from_file_location("check_trace2skill_approval_packet", path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot import {path}")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def approval_packet_errors(ara_root: Path) -> list[str]:
    plan_path = ara_root / "results/full_run_plan.md"
    if not plan_path.is_file():
        return [f"missing approval packet: {plan_path.relative_to(ara_root)}"]
    try:
        checker = import_approval_packet_checker()
        packet = checker.approval_packet(plan_path.read_text(encoding="utf-8"))
    except (ModuleNotFoundError, RuntimeError, ValueError) as exc:
        return [str(exc)]
    return checker.packet_errors(packet)


def is_actual_ara_result_path(ara_root: Path, path: Path) -> bool:
    try:
        return path.resolve().parent == (ara_root / "results").resolve()
    except OSError:
        return False


def is_trace2skill_ara_result_path(repo_root: Path, path: Path) -> bool:
    try:
        return path.resolve().parent == (
            repo_root / "docs/ara/trace2skill_spreadsheetbench/results"
        ).resolve()
    except OSError:
        return False


def is_json_number(value: Any) -> bool:
    return isinstance(value, (int, float)) and not isinstance(value, bool) and math.isfinite(value)


def check_non_empty_string(prefix: str, record: dict[str, Any], key: str, errors: list[str]) -> None:
    if not isinstance(record.get(key), str) or not record.get(key):
        errors.append(f"{prefix} {key} must be a non-empty string")


def check_base_record_shape(prefix: str, record: dict[str, Any], errors: list[str]) -> None:
    if record.get("schema_version") != RESULT_SCHEMA_VERSION:
        errors.append(f"{prefix} schema_version must be {RESULT_SCHEMA_VERSION!r}")

    for key in ("run_id", "created_at", "model_id", "serving_backend", "metric_name", "source_command"):
        check_non_empty_string(prefix, record, key, errors)

    proof = record.get("proof_classification")
    if proof not in ALLOWED_PROOF_CLASSIFICATIONS:
        errors.append(
            f"{prefix} proof_classification must be one of {sorted(ALLOWED_PROOF_CLASSIFICATIONS)!r}"
        )

    dataset_slice = record.get("dataset_slice")
    if not isinstance(dataset_slice, dict):
        errors.append(f"{prefix} dataset_slice must be an object")
    else:
        for key in ("name", "split", "denominator"):
            if not isinstance(dataset_slice.get(key), str) or not dataset_slice.get(key):
                errors.append(f"{prefix} dataset_slice.{key} must be a non-empty string")
        case_count = dataset_slice.get("case_count")
        if not isinstance(case_count, int) or isinstance(case_count, bool) or case_count < 0:
            errors.append(f"{prefix} dataset_slice.case_count must be a non-negative integer")

    seed = record.get("seed")
    if seed is not None and not (
        (isinstance(seed, str) and seed)
        or (isinstance(seed, (int, float)) and not isinstance(seed, bool) and math.isfinite(seed))
    ):
        errors.append(f"{prefix} seed must be a number, non-empty string, or null")

    skill_source = record.get("skill_source")
    if not isinstance(skill_source, dict):
        errors.append(f"{prefix} skill_source must be an object")
    elif not isinstance(skill_source.get("kind"), str) or not skill_source.get("kind"):
        errors.append(f"{prefix} skill_source.kind must be a non-empty string")

    if not is_json_number(record.get("metric_value")):
        errors.append(f"{prefix} metric_value must be numeric")

    metric_unit = record.get("metric_unit")
    if metric_unit not in ALLOWED_METRIC_UNITS:
        errors.append(f"{prefix} metric_unit must be one of {sorted(ALLOWED_METRIC_UNITS)!r}")

    binding = record.get("plot_binding")
    if binding is not None:
        if not isinstance(binding, dict):
            errors.append(f"{prefix} plot_binding must be an object or null")
        else:
            for key in ("panel", "x_label", "series", "axis"):
                if not isinstance(binding.get(key), str) or not binding.get(key):
                    errors.append(f"{prefix} plot_binding.{key} must be a non-empty string")

    cost = record.get("cost")
    if not isinstance(cost, dict):
        errors.append(f"{prefix} cost must be an object")
    else:
        for key in ("usd", "prompt_tokens", "completion_tokens"):
            if key in cost and cost[key] is not None and not is_json_number(cost[key]):
                errors.append(f"{prefix} cost.{key} must be numeric or null")

    runtime = record.get("runtime")
    if not isinstance(runtime, dict):
        errors.append(f"{prefix} runtime must be an object")
    else:
        if "seconds" not in runtime:
            errors.append(f"{prefix} runtime.seconds must be present")
        elif runtime["seconds"] is not None and not is_json_number(runtime["seconds"]):
            errors.append(f"{prefix} runtime.seconds must be numeric or null")
        for key in ("workers", "max_turns"):
            if key in runtime and runtime[key] is not None:
                value = runtime[key]
                if not isinstance(value, int) or isinstance(value, bool) or value < 0:
                    errors.append(f"{prefix} runtime.{key} must be a non-negative integer or null")

    if "artifact_paths" not in record:
        errors.append(f"{prefix} artifact_paths must be a non-empty list")

    extra = record.get("extra")
    if not isinstance(extra, dict):
        errors.append(f"{prefix} extra must be an object")

    if not isinstance(record.get("notes"), str):
        errors.append(f"{prefix} notes must be a string")


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


def stage_required_file_artifact_patterns(stage: dict[str, Any]) -> list[tuple[str, re.Pattern[str]]]:
    expected = stage.get("expected_artifacts")
    if not isinstance(expected, list):
        return []
    patterns: list[tuple[str, re.Pattern[str]]] = []
    for artifact in expected:
        artifact_text = str(artifact)
        if Path(artifact_text).name in OPTIONAL_ROW_ARTIFACT_FRAGMENTS:
            continue
        if " " in artifact_text:
            continue
        if "." not in Path(artifact_text).name:
            continue
        pattern_text = re.escape(artifact_text)
        pattern_text = re.sub(r"\\\{[^}]+\\\}", r"[^/]+", pattern_text)
        pattern_text = re.sub(r"<[^>]+>", r"[^/]+", pattern_text)
        patterns.append((artifact_text, re.compile(pattern_text)))
    return patterns


def check_required_stage_file_artifacts(
    prefix: str,
    stage: dict[str, Any] | None,
    artifact_paths: Any,
    errors: list[str],
) -> None:
    if stage is None or stage.get("expected_command_policy") is None:
        return
    if not isinstance(artifact_paths, list):
        return
    for source, pattern in stage_required_file_artifact_patterns(stage):
        if not any(pattern.search(str(path)) for path in artifact_paths):
            errors.append(f"{prefix} missing artifact matching runbook expectation {source!r}")


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
    record: dict[str, Any],
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
    split = dataset_slice.get("split")
    case_range = dataset_slice.get("case_range")
    case_count = dataset_slice.get("case_count")
    expected_split = expected.get("split")

    if expected_split is not None and split != expected_split:
        errors.append(f"{prefix} {stage_id} rows must use dataset_slice.split {expected_split!r}")

    if kind == "one-case":
        if case_range != expected.get("case_range"):
            errors.append(f"{prefix} {stage_id} rows must use dataset_slice.case_range {expected.get('case_range')!r}")
        if case_count != expected.get("case_count"):
            errors.append(f"{prefix} {stage_id} rows must use dataset_slice.case_count {expected.get('case_count')}")
        required = expected.get("denominator_contains")
        if not isinstance(denominator, str) or not isinstance(required, str) or required not in denominator:
            errors.append(f"{prefix} {stage_id} rows must name a denominator containing {required!r}")
        expected_case_id = expected.get("case_id")
        if expected_case_id is not None:
            extra = record.get("extra")
            case_id = extra.get("case_id") if isinstance(extra, dict) else None
            if case_id != expected_case_id:
                errors.append(f"{prefix} {stage_id} rows must carry extra.case_id {expected_case_id!r}")
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

    if kind in {"aggregate", "full-paper"}:
        if case_range != expected.get("case_range"):
            errors.append(f"{prefix} {stage_id} rows must use dataset_slice.case_range {expected.get('case_range')!r}")
        if case_count != expected.get("case_count"):
            errors.append(f"{prefix} {stage_id} rows must use dataset_slice.case_count {expected.get('case_count')}")
        if denominator != expected.get("denominator"):
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


def check_source_identity_matches(
    prefix: str,
    stage_id: Any,
    parent: dict[str, Any],
    source: dict[str, Any],
    source_path: Path,
    source_line_number: int,
    repo_root: Path,
    errors: list[str],
) -> None:
    source_label = f"{source_path.relative_to(repo_root)}:{source_line_number}"
    for key, noun in (
        ("model_id", "model_id"),
        ("serving_backend", "serving_backend"),
        ("metric_name", "metric_name"),
        ("metric_unit", "metric_unit"),
    ):
        if source.get(key) != parent.get(key):
            errors.append(
                f"{prefix} {stage_id} source row {source_label} must match parent row {noun} {parent.get(key)!r}"
            )


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
    source_approval_blockers: list[str] | None = None,
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
        metric_values_by_seed: dict[int, float] = {}
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
                approval_blockers=source_approval_blockers
                if is_trace2skill_ara_result_path(repo_root, source_path)
                else None,
                source_approval_blockers=source_approval_blockers,
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
            check_source_identity_matches(
                prefix,
                stage_id,
                record,
                source,
                source_path,
                line_number,
                repo_root,
                errors,
            )
            seed = normalize_seed(source.get("seed"))
            if seed is not None:
                observed.add(seed)
                metric_value = source.get("metric_value")
                if is_json_number(metric_value):
                    if seed in metric_values_by_seed:
                        errors.append(f"{prefix} {stage_id} aggregate rows must cite at most one metric row for seed {seed}")
                    else:
                        metric_values_by_seed[seed] = float(metric_value)
        missing = [seed for seed in required_seeds if seed not in observed]
        if missing:
            errors.append(f"{prefix} {stage_id} aggregate rows must cite source result rows for seeds {missing!r}")
        missing_metric_values = [seed for seed in required_seeds if seed not in metric_values_by_seed]
        if missing_metric_values:
            errors.append(
                f"{prefix} {stage_id} aggregate rows must cite numeric source metric_values for seeds {missing_metric_values!r}"
            )
        elif is_json_number(record.get("metric_value")):
            expected_metric = sum(metric_values_by_seed[seed] for seed in required_seeds) / len(required_seeds)
            if not math.isclose(float(record["metric_value"]), expected_metric, rel_tol=0, abs_tol=1e-9):
                errors.append(
                    f"{prefix} {stage_id} aggregate metric_value must equal mean source metric_value {expected_metric}"
                )
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
            errors.append(
                f"{prefix} {stage_id} full-paper rows must cite training-validation, "
                "seed-aggregate, or paper-candidate source result rows"
            )
            return
        dataset_expected = stage.get("expected_dataset_slice")
        required_ranges = (
            dataset_expected.get("required_split_ranges")
            if isinstance(dataset_expected, dict)
            else None
        )
        observed_ranges: set[str] = set()
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
                approval_blockers=source_approval_blockers
                if is_trace2skill_ara_result_path(repo_root, source_path)
                else None,
                source_approval_blockers=source_approval_blockers,
            )
            if source_errors:
                errors.append(
                    f"{prefix} {stage_id} source row {source_path.relative_to(repo_root)}:{source_line_number} "
                    f"does not pass result intake: {source_errors!r}"
                )
            source_slice = source.get("dataset_slice")
            source_range = source_slice.get("case_range") if isinstance(source_slice, dict) else None
            if isinstance(source_range, str) and source_range:
                observed_ranges.add(source_range)
            check_source_identity_matches(
                prefix,
                stage_id,
                record,
                source,
                source_path,
                source_line_number,
                repo_root,
                errors,
            )
        if isinstance(required_ranges, list) and all(isinstance(item, str) for item in required_ranges):
            missing = [case_range for case_range in required_ranges if case_range not in observed_ranges]
            if missing:
                errors.append(
                    f"{prefix} {stage_id} full-paper rows must cite source result rows covering split ranges {missing!r}"
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
    approval_blockers: list[str] | None = None,
    source_approval_blockers: list[str] | None = None,
) -> None:
    prefix = f"{path.relative_to(repo_root)}:{line_number}"
    check_base_record_shape(prefix, record, errors)
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
    check_stage_dataset_slice(prefix, stage, record, dataset_slice, errors)
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
            source_approval_blockers=source_approval_blockers,
        )
    check_required_prompt_artifacts(prefix, proof, stage, artifact_paths, errors)
    check_required_stage_file_artifacts(prefix, stage, artifact_paths, errors)

    skill_source = record.get("skill_source")
    skill_path = skill_source.get("path") if isinstance(skill_source, dict) else None
    if skill_path is not None:
        check_artifact_path(repo_root, prefix, skill_path, errors)
        if isinstance(artifact_paths, list) and skill_path not in artifact_paths:
            errors.append(f"{prefix} skill_source.path {skill_path!r} must also appear in artifact_paths")

    if proof in APPROVAL_REQUIRED_PROOF_CLASSIFICATIONS:
        if approval_blockers:
            errors.append(
                f"{prefix} {proof} rows require a runnable approval packet; "
                f"blocked by {len(approval_blockers)} approval blocker(s)"
            )
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

    if proof in PAPER_DENOMINATOR_CLASSIFICATIONS:
        case_count = record.get("dataset_slice", {}).get("case_count")
        if not isinstance(case_count, int) or case_count < 200:
            errors.append(f"{prefix} paper-denominator rows must cover at least the 200-case paper split")
        if record.get("serving_backend") != "vLLM":
            errors.append(f"{prefix} paper-denominator rows must use vLLM")
        if record.get("model_id") not in {"Qwen3.5-122B-A10B", "Qwen3.5-35B-A3B"}:
            errors.append(f"{prefix} paper-denominator row has non-paper model_id")

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
    approval_blockers = approval_packet_errors(ara_root)
    for path in sorted(results_dir.glob("*.jsonl")):
        try:
            rows = load_jsonl(path)
        except (json.JSONDecodeError, ValueError) as exc:
            errors.append(str(exc))
            continue
        for line_number, record in rows:
            check_record(
                repo_root,
                path,
                line_number,
                record,
                runbook_stages,
                errors,
                approval_blockers=approval_blockers,
                source_approval_blockers=approval_blockers,
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
    errors = check_result_intake(repo_root, ara_root)
    if errors:
        for error in errors:
            print(f"ERROR: {error}", file=sys.stderr)
        return 1
    print(f"PASS: {args.ara_dir} result intake")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
