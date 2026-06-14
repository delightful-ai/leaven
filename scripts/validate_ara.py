#!/usr/bin/env python3
"""Seal Level 1 structural validator for local ARA packages."""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path
from typing import Any

import yaml


MANDATORY_DIRS = [
    "logic",
    "logic/solution",
    "src",
    "src/configs",
    "src/execution",
    "trace",
    "evidence",
]

MANDATORY_FILES = [
    "PAPER.md",
    "logic/problem.md",
    "logic/claims.md",
    "logic/concepts.md",
    "logic/experiments.md",
    "logic/solution/architecture.md",
    "logic/solution/algorithm.md",
    "logic/solution/constraints.md",
    "logic/solution/heuristics.md",
    "logic/related_work.md",
    "src/configs/training.md",
    "src/configs/model.md",
    "src/environment.md",
    "trace/exploration_tree.yaml",
    "evidence/README.md",
]


def fail(errors: list[str], message: str) -> None:
    errors.append(message)


def read(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def frontmatter(markdown: str) -> dict[str, Any]:
    match = re.match(r"^---\n(.*?)\n---\n", markdown, re.S)
    if not match:
        return {}
    loaded = yaml.safe_load(match.group(1))
    return loaded if isinstance(loaded, dict) else {}


def markdown_ids(pattern: str, text: str) -> set[str]:
    return set(re.findall(pattern, text, re.M))


def iter_tree_nodes(node: Any) -> list[dict[str, Any]]:
    nodes: list[dict[str, Any]] = []
    if isinstance(node, dict):
        if "id" in node:
            nodes.append(node)
        for child in node.get("children", []) or []:
            nodes.extend(iter_tree_nodes(child))
    return nodes


def validate(root: Path) -> list[str]:
    errors: list[str] = []

    for rel in MANDATORY_DIRS:
        if not (root / rel).is_dir():
            fail(errors, f"missing directory: {rel}")

    for rel in MANDATORY_FILES:
        path = root / rel
        if not path.is_file():
            fail(errors, f"missing file: {rel}")
        elif not read(path).strip():
            fail(errors, f"empty file: {rel}")

    execution_files = sorted((root / "src/execution").glob("*.py"))
    if not execution_files:
        fail(errors, "missing execution stub: src/execution/*.py")

    paper_path = root / "PAPER.md"
    if paper_path.is_file():
        paper = read(paper_path)
        fm = frontmatter(paper)
        for key in ("title", "authors", "year"):
            if key not in fm:
                fail(errors, f"PAPER.md frontmatter missing {key}")
        if "## Layer Index" not in paper:
            fail(errors, "PAPER.md missing Layer Index")

    claims_path = root / "logic/claims.md"
    experiments_path = root / "logic/experiments.md"
    concepts_path = root / "logic/concepts.md"
    heuristics_path = root / "logic/solution/heuristics.md"

    claims = read(claims_path) if claims_path.is_file() else ""
    experiments = read(experiments_path) if experiments_path.is_file() else ""
    concepts = read(concepts_path) if concepts_path.is_file() else ""
    heuristics = read(heuristics_path) if heuristics_path.is_file() else ""

    claim_ids = markdown_ids(r"^## (C\d+):", claims)
    experiment_ids = markdown_ids(r"^## (E\d+):", experiments)

    if "C01" not in claim_ids:
        fail(errors, "claims.md missing C01")
    if "E01" not in experiment_ids:
        fail(errors, "experiments.md missing E01")
    if len(markdown_ids(r"^## .+", concepts)) < 5:
        fail(errors, "concepts.md has fewer than five concepts")
    if len(experiment_ids) < 3:
        fail(errors, "experiments.md has fewer than three experiments")
    if "## H01:" not in heuristics:
        fail(errors, "heuristics.md missing H01")

    for claim_id in claim_ids:
        block_match = re.search(rf"^## {claim_id}:.*?(?=^## C\d+:|\Z)", claims, re.M | re.S)
        block = block_match.group(0) if block_match else ""
        for field in ("Statement", "Status", "Falsification criteria", "Proof"):
            if f"**{field}**" not in block:
                fail(errors, f"{claim_id} missing {field}")
        proof_match = re.search(r"\*\*Proof\*\*:\s*\[([^\]]*)\]", block)
        if proof_match:
            for proof in re.findall(r"E\d+", proof_match.group(1)):
                if proof not in experiment_ids:
                    fail(errors, f"{claim_id} references unknown experiment {proof}")

    for experiment_id in experiment_ids:
        block_match = re.search(rf"^## {experiment_id}:.*?(?=^## E\d+:|\Z)", experiments, re.M | re.S)
        block = block_match.group(0) if block_match else ""
        for field in ("Verifies", "Setup", "Procedure", "Expected outcome"):
            if f"**{field}**" not in block:
                fail(errors, f"{experiment_id} missing {field}")
        verifies_match = re.search(r"\*\*Verifies\*\*:\s*([^\n]+)", block)
        if verifies_match:
            for claim in re.findall(r"C\d+", verifies_match.group(1)):
                if claim not in claim_ids:
                    fail(errors, f"{experiment_id} references unknown claim {claim}")

    for code_ref in re.findall(r"\*\*Code ref\*\*:\s*`([^`]+)`", heuristics):
        if not (root / code_ref).exists():
            fail(errors, f"heuristic code ref does not exist: {code_ref}")

    tree_path = root / "trace/exploration_tree.yaml"
    if tree_path.is_file():
        try:
            tree = yaml.safe_load(read(tree_path))
        except yaml.YAMLError as exc:
            fail(errors, f"exploration_tree.yaml failed YAML parse: {exc}")
            tree = None
        nodes = iter_tree_nodes(tree.get("root") if isinstance(tree, dict) else tree)
        node_types = {str(node.get("type")) for node in nodes}
        if len(nodes) < 8:
            fail(errors, "exploration_tree.yaml has fewer than eight nodes")
        if "dead_end" not in node_types:
            fail(errors, "exploration_tree.yaml missing dead_end node")
        if "decision" not in node_types:
            fail(errors, "exploration_tree.yaml missing decision node")
        for node in nodes:
            support = node.get("support_level")
            if support not in {"explicit", "inferred"}:
                fail(errors, f"trace node {node.get('id')} missing valid support_level")
            if support == "explicit" and not node.get("sources"):
                fail(errors, f"explicit trace node {node.get('id')} missing sources")

    evidence_files = sorted(
        path
        for path in (root / "evidence").rglob("*.md")
        if path.name != "README.md"
    )
    if not evidence_files:
        fail(errors, "no evidence files found")
    for evidence in evidence_files:
        text = read(evidence)
        if "**Source**" not in text:
            fail(errors, f"{evidence.relative_to(root)} missing **Source** field")
        if "|" not in text:
            fail(errors, f"{evidence.relative_to(root)} missing Markdown table")

    return errors


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("ara_dir", type=Path)
    args = parser.parse_args()
    root = args.ara_dir

    errors = validate(root)
    if errors:
        for error in errors:
            print(f"FAIL: {error}", file=sys.stderr)
        return 1

    file_count = sum(1 for path in root.rglob("*") if path.is_file())
    print(f"PASS: {root} ({file_count} files)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
