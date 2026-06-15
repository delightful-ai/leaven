#!/usr/bin/env python3
"""Check the Trace2Skill full-denominator approval packet."""

from __future__ import annotations

import argparse
import re
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

import yaml

EXPECTED_EXACT_VALUES: dict[tuple[str, ...], Any] = {
    ("serving", "backend"): "vLLM",
    ("dataset", "path"): "data/spreadsheetbench_verified/spreadsheetbench_verified_400",
    ("protocol", "seeds"): [41, 42, 43],
    ("protocol", "stage2_workers"): 128,
    ("protocol", "merge_batch_size"): 32,
    ("protocol", "react_turn_budget"): 100,
    ("tolerance", "approved"): True,
}

EXPECTED_FILLED_VALUES: dict[tuple[str, ...], Any] = {
    ("models", "qwen_122b"): "Qwen3.5-122B-A10B",
    ("models", "qwen_35b"): "Qwen3.5-35B-A3B",
}

REQUIRED_APPROVAL_FIELDS = [
    ("models", "qwen_122b"),
    ("models", "qwen_35b"),
    ("serving", "host"),
    ("serving", "version"),
    ("serving", "tensor_parallel"),
    ("serving", "gpu_type"),
    ("serving", "gpu_count"),
    ("dataset", "checksum_or_manifest"),
    ("budget", "max_usd"),
    ("budget", "max_wall_clock_hours"),
    ("budget", "max_gpu_hours"),
    ("credentials", "api_key_env"),
    ("credentials", "redaction_policy"),
    ("credentials", "log_retention"),
    ("artifacts", "root"),
    ("artifacts", "retention"),
    ("approval", "approved_by"),
    ("approval", "approved_at"),
]

REQUIRED_ARTIFACT_HINTS = [
    "run_metadata",
    "manifest",
    "trajectory",
    "score_report",
    "skill",
    "leaven_results",
]

POSITIVE_INTEGER_FIELDS = [
    ("serving", "tensor_parallel"),
    ("serving", "gpu_count"),
]

POSITIVE_NUMBER_FIELDS = [
    ("budget", "max_usd"),
    ("budget", "max_wall_clock_hours"),
    ("budget", "max_gpu_hours"),
]

REFERENCE_PATH_FIELDS = [
    ("dataset", "checksum_or_manifest"),
    ("tolerance", "policy"),
]


def approval_packet(markdown: str) -> dict[str, Any]:
    header = "## Approval Packet To Collect Before Running"
    if header not in markdown:
        raise ValueError("full_run_plan.md missing approval packet header")
    after_header = markdown.split(header, 1)[1]
    match = re.search(r"```yaml\n(.*?)\n```", after_header, re.S)
    if not match:
        raise ValueError("full_run_plan.md missing fenced yaml approval packet")
    loaded = yaml.safe_load(match.group(1))
    if not isinstance(loaded, dict):
        raise ValueError("approval packet YAML must parse to an object")
    return loaded


def lookup(packet: dict[str, Any], path: tuple[str, ...]) -> Any:
    cursor: Any = packet
    for part in path:
        if not isinstance(cursor, dict) or part not in cursor:
            return None
        cursor = cursor[part]
    return cursor


def unresolved(value: Any) -> bool:
    if value is None:
        return True
    if isinstance(value, str):
        stripped = value.strip()
        if not stripped or stripped.lower() in {"null", "todo", "tbd", "pending"}:
            return True
        if "<" in stripped or ">" in stripped:
            return True
    return False


def is_utc_approval_timestamp(value: Any) -> bool:
    if isinstance(value, str):
        return bool(re.fullmatch(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z", value))
    if isinstance(value, datetime):
        return value.tzinfo is not None and value.utcoffset() == timezone.utc.utcoffset(None)
    return False


def packet_errors(packet: dict[str, Any], ara_root: Path | None = None) -> list[str]:
    errors: list[str] = []
    for path in REQUIRED_APPROVAL_FIELDS:
        value = lookup(packet, path)
        if unresolved(value):
            errors.append(f"{'.'.join(path)} is unresolved")

    for path, expected in EXPECTED_EXACT_VALUES.items():
        actual = lookup(packet, path)
        if actual != expected:
            errors.append(f"{'.'.join(path)} must be {expected!r}, got {actual!r}")

    for path, expected in EXPECTED_FILLED_VALUES.items():
        actual = lookup(packet, path)
        if not unresolved(actual) and actual != expected:
            errors.append(f"{'.'.join(path)} must be {expected!r}, got {actual!r}")

    for path in POSITIVE_INTEGER_FIELDS:
        value = lookup(packet, path)
        if not unresolved(value) and (
            not isinstance(value, int) or isinstance(value, bool) or value <= 0
        ):
            errors.append(f"{'.'.join(path)} must be a positive integer, got {value!r}")

    for path in POSITIVE_NUMBER_FIELDS:
        value = lookup(packet, path)
        if not unresolved(value) and (
            not isinstance(value, int | float) or isinstance(value, bool) or value <= 0
        ):
            errors.append(f"{'.'.join(path)} must be a positive number, got {value!r}")

    approved_at = lookup(packet, ("approval", "approved_at"))
    if not unresolved(approved_at):
        if not is_utc_approval_timestamp(approved_at):
            errors.append("approval.approved_at must be UTC ISO-8601 like 2026-06-14T12:00:00Z")

    api_key_env = lookup(packet, ("credentials", "api_key_env"))
    if not unresolved(api_key_env):
        if not isinstance(api_key_env, str) or not re.fullmatch(
            r"[A-Z_][A-Z0-9_]*", api_key_env
        ):
            errors.append("credentials.api_key_env must be an environment variable name")

    if ara_root is not None:
        repo_root = ara_root.parents[2] if len(ara_root.parents) >= 3 else ara_root
        for path in REFERENCE_PATH_FIELDS:
            value = lookup(packet, path)
            if unresolved(value):
                continue
            if not isinstance(value, str):
                errors.append(f"{'.'.join(path)} must be a relative path string, got {value!r}")
                continue
            candidate = repo_root / value
            if not candidate.is_file():
                errors.append(f"{'.'.join(path)} path does not exist: {value}")

    expected_artifacts = lookup(packet, ("artifacts", "expected"))
    if not isinstance(expected_artifacts, list) or not expected_artifacts:
        errors.append("artifacts.expected must be a non-empty list")
    else:
        joined = "\n".join(str(item) for item in expected_artifacts)
        for hint in REQUIRED_ARTIFACT_HINTS:
            if hint not in joined:
                errors.append(f"artifacts.expected missing {hint!r} artifact")

    return errors


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("ara_dir", type=Path)
    parser.add_argument(
        "--expect-blocked",
        action="store_true",
        help="Succeed only when the packet is still blocked by unresolved approval fields.",
    )
    args = parser.parse_args()

    plan_path = args.ara_dir / "results/full_run_plan.md"
    if not plan_path.is_file():
        print(f"FAIL: missing {plan_path}", file=sys.stderr)
        return 1

    try:
        packet = approval_packet(plan_path.read_text(encoding="utf-8"))
    except ValueError as exc:
        print(f"FAIL: {exc}", file=sys.stderr)
        return 1

    errors = packet_errors(packet, args.ara_dir.resolve())
    if args.expect_blocked:
        if errors:
            print("BLOCKED: approval packet is not runnable")
            for error in errors:
                print(f"- {error}")
            return 0
        print("FAIL: approval packet is runnable but --expect-blocked was set", file=sys.stderr)
        return 1

    if errors:
        print("FAIL: approval packet is not runnable", file=sys.stderr)
        for error in errors:
            print(f"- {error}", file=sys.stderr)
        return 1

    print("PASS: approval packet is runnable")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
