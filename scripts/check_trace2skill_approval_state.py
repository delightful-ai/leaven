#!/usr/bin/env python3
"""Check Trace2Skill approval-packet state against closeout/status docs."""

from __future__ import annotations

import argparse
import importlib.util
import json
import sys
from pathlib import Path
from typing import Any


def import_approval_checker(repo_root: Path) -> Any:
    checker_path = repo_root / "scripts/check_trace2skill_approval_packet.py"
    spec = importlib.util.spec_from_file_location("check_trace2skill_approval_packet", checker_path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot import {checker_path}")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def read(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def check_blocked_state(ara_root: Path, closeout: dict[str, Any], packet_errors: list[str]) -> list[str]:
    errors: list[str] = []
    status = closeout.get("acceptance", {}).get("full_denominator_plan_approved", {})
    remaining = status.get("remaining")
    if status.get("status") != "blocked":
        errors.append("closeout full_denominator_plan_approved status must be blocked while packet has errors")
    if not isinstance(remaining, list):
        errors.append("closeout full_denominator_plan_approved remaining must be a list")
        remaining = []
    for packet_error in packet_errors:
        if packet_error not in remaining:
            errors.append(f"closeout missing approval blocker: {packet_error}")
    if closeout.get("overall_complete") is not False:
        errors.append("closeout overall_complete must be false while approval packet is blocked")

    closeout_md = read(ara_root / "results/closeout_audit.md")
    if "approval preflight is blocked" not in closeout_md:
        errors.append("closeout_audit.md must state approval preflight is blocked")

    denominator_status = read(ara_root / "results/denominator_status.md")
    required_fragments = [
        "## Approval Blocker",
        "expected to fail in normal",
        "Do not launch Qwen/vLLM-scale execution",
    ]
    for fragment in required_fragments:
        if fragment not in denominator_status:
            errors.append(f"denominator_status.md missing blocked-approval fragment: {fragment}")
    return errors


def check_runnable_state(ara_root: Path, closeout: dict[str, Any]) -> list[str]:
    errors: list[str] = []
    status = closeout.get("acceptance", {}).get("full_denominator_plan_approved", {})
    if status.get("status") == "blocked":
        errors.append("closeout full_denominator_plan_approved must not stay blocked after packet becomes runnable")
    for rel_path in ["results/closeout_audit.md", "results/denominator_status.md"]:
        text = read(ara_root / rel_path)
        if "approval preflight is blocked" in text or "expected to fail in normal" in text:
            errors.append(f"{rel_path} still describes blocked approval after packet becomes runnable")
    return errors


def check_approval_state(repo_root: Path, ara_root: Path) -> list[str]:
    errors: list[str] = []
    approval = import_approval_checker(repo_root)

    plan_path = ara_root / "results/full_run_plan.md"
    closeout_path = ara_root / "results/closeout_audit.json"
    closeout_md_path = ara_root / "results/closeout_audit.md"
    denominator_path = ara_root / "results/denominator_status.md"
    for path in [plan_path, closeout_path, closeout_md_path, denominator_path]:
        if not path.is_file():
            return [f"missing approval-state input: {path}"]

    packet = approval.approval_packet(read(plan_path))
    packet_errors = approval.packet_errors(packet)
    closeout = json.loads(read(closeout_path))

    if packet_errors:
        errors.extend(check_blocked_state(ara_root, closeout, packet_errors))
    else:
        errors.extend(check_runnable_state(ara_root, closeout))
    return errors


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("ara_dir", type=Path)
    args = parser.parse_args()

    ara_root = args.ara_dir.resolve()
    repo_root = Path(__file__).resolve().parents[1]
    errors = check_approval_state(repo_root, ara_root)
    if errors:
        for error in errors:
            print(f"ERROR: {error}")
        return 1
    print(f"PASS: {args.ara_dir} approval state")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
