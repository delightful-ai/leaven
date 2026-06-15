#!/usr/bin/env python3
"""Validate Trace2Skill ARA rigor-review follow-up claims."""

from __future__ import annotations

import argparse
import json
import re
from pathlib import Path
from typing import Any

import yaml


ADDRESSED_FINDINGS = {"F02", "F04"}
PARTIAL_FINDINGS = {"F03", "F05"}


def repo_root_for(ara_root: Path) -> Path:
    for candidate in (ara_root, *ara_root.parents):
        if (candidate / "docs/ara/trace2skill_spreadsheetbench").is_dir():
            return candidate
    return Path.cwd()


def read(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def iter_tree_nodes(node: Any) -> list[dict[str, Any]]:
    nodes: list[dict[str, Any]] = []
    if isinstance(node, dict):
        if "id" in node:
            nodes.append(node)
        for child in node.get("children", []) or []:
            nodes.extend(iter_tree_nodes(child))
    return nodes


def c07_block(claims: str) -> str:
    match = re.search(r"^## C07:.*?(?=^## C\d+:|\Z)", claims, re.M | re.S)
    return match.group(0) if match else ""


def check_level2_followup(ara_root: Path) -> list[str]:
    errors: list[str] = []
    path = ara_root / "level2_report.json"
    if not path.is_file():
        return ["missing level2_report.json"]
    report = json.loads(read(path))
    followup = report.get("post_review_followup")
    if not isinstance(followup, dict):
        return ["level2_report.json missing post_review_followup"]

    addressed = set(followup.get("addressed_findings") or [])
    partial = set(followup.get("partially_addressed_findings") or [])
    if not ADDRESSED_FINDINGS.issubset(addressed):
        errors.append(f"level2_report.json addressed_findings must include {sorted(ADDRESSED_FINDINGS)}")
    if not PARTIAL_FINDINGS.issubset(partial):
        errors.append(f"level2_report.json partially_addressed_findings must include {sorted(PARTIAL_FINDINGS)}")
    blockers = followup.get("remaining_blockers")
    if not isinstance(blockers, list) or not blockers:
        errors.append("level2_report.json post_review_followup.remaining_blockers must be non-empty")
    for artifact in (
        "src/configs/tolerance.md",
        "evidence/prompt_templates.md",
        "trace/exploration_tree.yaml",
        "logic/claims.md",
    ):
        if artifact not in (followup.get("followup_artifacts") or []):
            errors.append(f"level2_report.json post_review_followup.followup_artifacts missing {artifact}")
    return errors


def check_dead_ends(ara_root: Path) -> list[str]:
    errors: list[str] = []
    path = ara_root / "trace/exploration_tree.yaml"
    if not path.is_file():
        return ["missing trace/exploration_tree.yaml"]
    tree = yaml.safe_load(read(path))
    nodes = iter_tree_nodes(tree.get("root") if isinstance(tree, dict) else tree)
    dead_ends = [node for node in nodes if node.get("type") == "dead_end"]
    if not dead_ends:
        errors.append("trace/exploration_tree.yaml must contain dead_end nodes")
    for node in dead_ends:
        for field in ("failure_mode", "lesson"):
            if not isinstance(node.get(field), str) or not node[field].strip():
                errors.append(f"dead_end {node.get('id')} missing non-empty {field}")
    return errors


def check_claim_followup(ara_root: Path) -> list[str]:
    path = ara_root / "logic/claims.md"
    if not path.is_file():
        return ["missing logic/claims.md"]
    block = c07_block(read(path))
    if not block:
        return ["logic/claims.md missing C07"]
    if "**Proof**: [E08, E09]" not in block:
        return ["C07 proof must cite both E08 and E09"]
    return []


def check_followup_artifacts(ara_root: Path) -> list[str]:
    errors: list[str] = []
    for rel in ("src/configs/tolerance.md", "evidence/prompt_templates.md"):
        path = ara_root / rel
        if not path.is_file() or not read(path).strip():
            errors.append(f"missing or empty {rel}")
    return errors


def check_review_markdown(ara_root: Path) -> list[str]:
    errors: list[str] = []
    path = ara_root / "reviews/rigor_review.md"
    if not path.is_file():
        return ["missing reviews/rigor_review.md"]
    text = read(path)
    for required in (
        "| F02 | Major | `trace/exploration_tree.yaml` has dead-end nodes but no explicit `failure_mode` or `lesson` fields. | Addressed after review. |",
        "| F04 | Minor | C07 cites only E08 even though E09 now owns the approval gate. | Addressed after review. |",
        "- `trace/exploration_tree.yaml` now includes `failure_mode` and `lesson` for",
        "- C07 now cites both E08 and E09.",
    ):
        if required not in text:
            errors.append(f"reviews/rigor_review.md missing follow-up text: {required}")
    stale_score_notes = (
        "C07 needs the E09 link.",
        "Real dead ends exist, but they need explicit failure/lesson fields.",
    )
    for stale in stale_score_notes:
        if stale in text:
            errors.append(f"reviews/rigor_review.md still contains stale score note: {stale}")
    return errors


def check_rigor_followup(repo_root: Path, ara_root: Path) -> list[str]:
    del repo_root
    errors: list[str] = []
    errors.extend(check_level2_followup(ara_root))
    errors.extend(check_dead_ends(ara_root))
    errors.extend(check_claim_followup(ara_root))
    errors.extend(check_followup_artifacts(ara_root))
    errors.extend(check_review_markdown(ara_root))
    return errors


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("ara_dir", type=Path)
    args = parser.parse_args()
    ara_root = args.ara_dir
    repo_root = repo_root_for(ara_root.resolve())

    errors = check_rigor_followup(repo_root, ara_root)
    if errors:
        for error in errors:
            print(f"FAIL: {error}")
        return 1
    print(f"PASS: {ara_root} rigor follow-up")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
