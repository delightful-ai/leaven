#!/usr/bin/env python3
"""Validate Trace2Skill runbook artifact expectations against denominator gates."""

from __future__ import annotations

import argparse
import json
import re
from pathlib import Path
from typing import Any

import yaml


APPROVAL_PACKET_REQUIRED_FRAGMENTS = {
    "run_metadata.json",
    "dataset_manifest.json",
    "trajectory_generation/{seed}/{case_id}/manifest.json",
    "trajectory_generation/{seed}/{case_id}/trajectory.json",
    "trajectory_generation/{seed}/{case_id}/score_report.json",
    "skill_evolution/{seed}/patch_pool.jsonl",
    "skill_evolution/{seed}/merge_tree.json",
    "skill_evolution/{seed}/skill/SKILL.md",
    "heldout_eval/{seed}/{case_id}/manifest.json",
    "heldout_eval/{seed}/{case_id}/trajectory.json",
    "heldout_eval/{seed}/{case_id}/score_report.json",
    "leaven_results.jsonl",
}

STAGE_REQUIRED_FRAGMENTS = {
    "G0": [
        "dataset_manifest.json",
        "closeout_audit.json",
        "validation.md",
    ],
    "G1": [
        "manifest.json",
        "13-1_output.xlsx",
        "acp_result.json",
        "agent_transcript.md",
        "score_report.json",
        "trajectory.json",
    ],
    "G1M": [
        "logs",
        "work",
        "outputs/eval_official_results.json",
        "leaven_results.jsonl",
    ],
    "G2": [
        "logs",
        "work",
        "outputs/eval_official_results.json",
        "leaven_results.jsonl",
    ],
    "G3": [
        "logs",
        "work",
        "outputs/eval_official_results.json",
        "error_analysis_parsed.json",
        "success_analysis_parsed.json",
        "change.log",
        "intermediates",
        "skills",
    ],
    "G3V": [
        "baseline_outputs/eval_official_results.json",
        "evolved_outputs/eval_official_results.json",
        "best_seed_selection_note.md",
    ],
    "G4": [
        "logs",
        "work",
        "outputs/eval_official_results.json",
        "leaven_results.jsonl",
    ],
    "G5": [
        "results/<approved-run-id>.jsonl",
        "trace2skill_targets.png",
        "closeout_audit.json",
    ],
    "G6": [
        "denominator-labeled result JSONL rows",
        "closeout_audit.json",
    ],
}


def repo_root_for(ara_root: Path) -> Path:
    for candidate in (ara_root, *ara_root.parents):
        if (candidate / "docs/ara/trace2skill_spreadsheetbench").is_dir():
            return candidate
    return Path.cwd()


def read(path: Path) -> str:
    return path.read_text(encoding="utf-8")


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


def load_runbook(ara_root: Path) -> dict[str, Any]:
    loaded = json.loads(read(ara_root / "results/full_denominator_runbook.json"))
    if not isinstance(loaded, dict):
        raise ValueError("full_denominator_runbook.json must parse to an object")
    return loaded


def joined_expected(stage: dict[str, Any]) -> str:
    expected = stage.get("expected_artifacts")
    if not isinstance(expected, list):
        return ""
    return "\n".join(str(item) for item in expected)


def check_fragments(name: str, actual_text: str, required: list[str] | set[str]) -> list[str]:
    return [f"{name} missing required artifact fragment {fragment!r}" for fragment in required if fragment not in actual_text]


def check_artifact_contract(repo_root: Path, ara_root: Path) -> list[str]:
    del repo_root
    errors: list[str] = []

    try:
        packet = approval_packet(read(ara_root / "results/full_run_plan.md"))
    except ValueError as exc:
        return [str(exc)]
    expected_artifacts = packet.get("artifacts", {}).get("expected")
    if not isinstance(expected_artifacts, list):
        errors.append("approval packet artifacts.expected must be a list")
    else:
        errors.extend(
            check_fragments(
                "approval packet artifacts.expected",
                "\n".join(str(item) for item in expected_artifacts),
                APPROVAL_PACKET_REQUIRED_FRAGMENTS,
            )
        )

    runbook = load_runbook(ara_root)
    stages = {
        stage.get("id"): stage
        for stage in runbook.get("stages", [])
        if isinstance(stage, dict) and isinstance(stage.get("id"), str)
    }
    missing_stages = set(STAGE_REQUIRED_FRAGMENTS) - set(stages)
    for stage_id in sorted(missing_stages):
        errors.append(f"full_denominator_runbook.json missing stage {stage_id}")

    for stage_id, required in STAGE_REQUIRED_FRAGMENTS.items():
        stage = stages.get(stage_id)
        if stage is None:
            continue
        expected_text = joined_expected(stage)
        if not expected_text:
            errors.append(f"stage {stage_id} expected_artifacts must be a non-empty list")
            continue
        errors.extend(check_fragments(f"stage {stage_id} expected_artifacts", expected_text, required))

    return errors


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("ara_dir", type=Path)
    args = parser.parse_args()
    ara_root = args.ara_dir.resolve()
    repo_root = repo_root_for(ara_root)

    errors = check_artifact_contract(repo_root, ara_root)
    if errors:
        for error in errors:
            print(f"FAIL: {error}")
        return 1
    print(f"PASS: {args.ara_dir} artifact contract")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
