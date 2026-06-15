#!/usr/bin/env python3
"""Seal Level 1 structural validator for local ARA packages."""

from __future__ import annotations

import argparse
import json
import re
import sys
from pathlib import Path
from typing import Any

import yaml

RESULT_SCHEMA_VERSION = "leaven.trace2skill.result.v1"
ALLOWED_PROOF_CLASSIFICATIONS = {
    "mechanics-smoke",
    "deterministic-one-case",
    "paper-subset",
    "paper-denominator-candidate",
    "paper-denominator-reproduction",
}
ALLOWED_METRIC_UNITS = {"percent", "delta_points", "minutes"}
SUPPORTED_RESULT_PANELS = {
    "same_model_deepening_vrf",
    "avg_improvement",
    "parallel_vs_sequential",
    "reasoningbank",
}


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


def validate_result_record(errors: list[str], record: Any, rel_path: Path, line_number: int) -> None:
    prefix = f"{rel_path}:{line_number}"
    if not isinstance(record, dict):
        fail(errors, f"{prefix} must be a JSON object")
        return

    required = [
        "schema_version",
        "run_id",
        "created_at",
        "proof_classification",
        "dataset_slice",
        "model_id",
        "seed",
        "skill_source",
        "metric_name",
        "metric_value",
        "metric_unit",
        "plot_binding",
        "cost",
        "runtime",
        "source_command",
        "artifact_paths",
        "notes",
    ]
    missing = [field for field in required if field not in record]
    if missing:
        fail(errors, f"{prefix} missing fields: {', '.join(missing)}")
        return
    if record["schema_version"] != RESULT_SCHEMA_VERSION:
        fail(errors, f"{prefix} schema_version must be {RESULT_SCHEMA_VERSION}")
    if record["proof_classification"] not in ALLOWED_PROOF_CLASSIFICATIONS:
        fail(errors, f"{prefix} has invalid proof_classification")
    if record["metric_unit"] not in ALLOWED_METRIC_UNITS:
        fail(errors, f"{prefix} has invalid metric_unit")
    if not isinstance(record["metric_value"], (int, float)) or isinstance(record["metric_value"], bool):
        fail(errors, f"{prefix} metric_value must be numeric")

    for field in ("run_id", "created_at", "model_id", "metric_name", "source_command", "notes"):
        if not isinstance(record[field], str):
            fail(errors, f"{prefix} {field} must be a string")
    for field in ("run_id", "created_at", "model_id", "metric_name", "source_command"):
        if isinstance(record[field], str) and not record[field].strip():
            fail(errors, f"{prefix} {field} must be non-empty")

    dataset_slice = record["dataset_slice"]
    if not isinstance(dataset_slice, dict):
        fail(errors, f"{prefix} dataset_slice must be an object")
    else:
        for field in ("name", "split", "case_count", "denominator"):
            if field not in dataset_slice:
                fail(errors, f"{prefix} dataset_slice missing {field}")
        if "case_count" in dataset_slice and (
            not isinstance(dataset_slice["case_count"], int) or dataset_slice["case_count"] < 1
        ):
            fail(errors, f"{prefix} dataset_slice.case_count must be a positive integer")
        for field in ("name", "split", "denominator"):
            if field in dataset_slice and (
                not isinstance(dataset_slice[field], str) or not dataset_slice[field].strip()
            ):
                fail(errors, f"{prefix} dataset_slice.{field} must be a non-empty string")

    skill_source = record["skill_source"]
    if not isinstance(skill_source, dict) or not isinstance(skill_source.get("kind"), str) or not skill_source["kind"]:
        fail(errors, f"{prefix} skill_source.kind must be a non-empty string")
    for field in ("plot_binding", "cost", "runtime"):
        if not isinstance(record[field], dict):
            fail(errors, f"{prefix} {field} must be an object")

    binding = record["plot_binding"]
    if isinstance(binding, dict):
        for field in ("panel", "x_label", "series", "axis"):
            if not isinstance(binding.get(field), str) or not binding[field]:
                fail(errors, f"{prefix} plot_binding.{field} must be a non-empty string")
        if binding.get("panel") not in SUPPORTED_RESULT_PANELS:
            fail(errors, f"{prefix} plot_binding.panel is not supported")
        if binding.get("axis") not in {"left", "right"}:
            fail(errors, f"{prefix} plot_binding.axis must be left or right")

    artifacts = record["artifact_paths"]
    if not isinstance(artifacts, list) or not artifacts:
        fail(errors, f"{prefix} artifact_paths must be a non-empty array")
    elif any(not isinstance(path_entry, str) or not path_entry for path_entry in artifacts):
        fail(errors, f"{prefix} artifact_paths entries must be non-empty strings")


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

    results_dir = root / "results"
    if results_dir.exists():
        if not results_dir.is_dir():
            fail(errors, "results exists but is not a directory")
        else:
            for rel in ("results/README.md", "results/leaven_result_schema.md"):
                path = root / rel
                if not path.is_file() or not read(path).strip():
                    fail(errors, f"missing or empty result schema file: {rel}")
            for jsonl_path in sorted(results_dir.glob("*.jsonl")):
                for line_number, line in enumerate(read(jsonl_path).splitlines(), start=1):
                    if not line.strip():
                        continue
                    try:
                        record = json.loads(line)
                    except json.JSONDecodeError as exc:
                        fail(errors, f"{jsonl_path.relative_to(root)}:{line_number} is not valid JSON: {exc}")
                        continue
                    validate_result_record(errors, record, jsonl_path.relative_to(root), line_number)

            closeout_audit_path = results_dir / "closeout_audit.json"
            if closeout_audit_path.exists():
                try:
                    closeout_audit = json.loads(read(closeout_audit_path))
                except json.JSONDecodeError as exc:
                    fail(errors, f"results/closeout_audit.json is not valid JSON: {exc}")
                    closeout_audit = None
                if isinstance(closeout_audit, dict):
                    if closeout_audit.get("schema_version") != "leaven.trace2skill.closeout_audit.v1":
                        fail(errors, "results/closeout_audit.json has wrong schema_version")
                    if closeout_audit.get("overall_complete") is not False:
                        fail(errors, "results/closeout_audit.json must keep overall_complete false until paper denominator is proven")
                    acceptance = closeout_audit.get("acceptance")
                    if not isinstance(acceptance, dict):
                        fail(errors, "results/closeout_audit.json missing acceptance object")
                    else:
                        for acceptance_id in (
                            "ara_level1_valid",
                            "plots_from_ara",
                            "current_mechanics_classified",
                            "one_case_live_or_explicit_blocker",
                            "full_denominator_plan_approved",
                            "reproduced_claim_limited_to_actual_denominator",
                        ):
                            if acceptance_id not in acceptance:
                                fail(errors, f"results/closeout_audit.json missing {acceptance_id}")

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
