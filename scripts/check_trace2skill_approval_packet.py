#!/usr/bin/env python3
"""Check the Trace2Skill full-denominator approval packet."""

from __future__ import annotations

import argparse
import re
import sys
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


def packet_errors(packet: dict[str, Any]) -> list[str]:
    errors: list[str] = []
    for path in REQUIRED_APPROVAL_FIELDS:
        value = lookup(packet, path)
        if unresolved(value):
            errors.append(f"{'.'.join(path)} is unresolved")

    for path, expected in EXPECTED_EXACT_VALUES.items():
        actual = lookup(packet, path)
        if actual != expected:
            errors.append(f"{'.'.join(path)} must be {expected!r}, got {actual!r}")

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

    errors = packet_errors(packet)
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
