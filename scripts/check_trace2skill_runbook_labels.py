#!/usr/bin/env python3
"""Validate Trace2Skill runbook labels against result proof classifications."""

from __future__ import annotations

import argparse
import ast
import json
import re
from pathlib import Path


RUNBOOK_ONLY_LABELS = {"guardrail-ready"}
EXPECTED_FORBIDDEN_LABELS = {
    "G0": "paper reproduction",
    "G1": "paper reproduction",
    "G1M": "held-out split reproduced",
    "G2": "held-out split reproduced",
    "G3": "held-out result",
    "G3V": "held-out result",
    "G4": "paper aggregate",
    "G5": "cross-model paper reproduction",
    "G6": "anything stronger than completed rows",
}
RESULT_PROOF_CLASSIFICATIONS = {
    "mechanics-smoke",
    "deterministic-one-case",
    "model-one-case",
    "paper-subset",
    "evolving-split-run",
    "training-validation-candidate",
    "held-out-single-seed-candidate",
    "seed-aggregate-candidate",
    "paper-denominator-candidate",
    "paper-denominator-reproduction",
}


def repo_root_for(ara_root: Path) -> Path:
    for candidate in (ara_root, *ara_root.parents):
        if (candidate / "docs/ara/trace2skill_spreadsheetbench").is_dir():
            return candidate
    return Path.cwd()


def read(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def load_runbook_stages(ara_root: Path) -> list[dict[str, object]]:
    runbook_path = ara_root / "results/full_denominator_runbook.json"
    runbook = json.loads(read(runbook_path))
    stages = runbook.get("stages")
    if not isinstance(stages, list):
        return []
    return [stage for stage in stages if isinstance(stage, dict)]


def extract_python_set(path: Path, name: str) -> set[str]:
    source = read(path)
    module = ast.parse(source)
    for node in module.body:
        if isinstance(node, ast.Assign):
            names = [target.id for target in node.targets if isinstance(target, ast.Name)]
            if name in names:
                value = ast.literal_eval(node.value)
                if isinstance(value, set) and all(isinstance(item, str) for item in value):
                    return set(value)
    raise ValueError(f"{path} missing literal set {name}")


def check_readme_labels(ara_root: Path, expected: set[str]) -> list[str]:
    errors: list[str] = []
    readme = read(ara_root / "results/README.md")
    for label in sorted(expected):
        if f"| `{label}` |" not in readme:
            errors.append(f"results/README.md missing proof classification row for {label}")
    return errors


def check_schema_notes(ara_root: Path) -> list[str]:
    errors: list[str] = []
    schema = read(ara_root / "results/leaven_result_schema.md")
    for label in ("model-one-case", "evolving-split-run", "training-validation-candidate"):
        if label not in schema:
            errors.append(f"results/leaven_result_schema.md missing non-overlay note for {label}")
    return errors


def check_plan_labels(ara_root: Path, expected: set[str]) -> list[str]:
    errors: list[str] = []
    plan = read(ara_root / "results/full_run_plan.md")
    labels = set(re.findall(r"`([^`]+)`", plan))
    expected_plan_labels = expected - {"mechanics-smoke", "paper-denominator-candidate"}
    missing = expected_plan_labels - labels
    if missing:
        errors.append(f"full_run_plan.md missing labels: {sorted(missing)}")
    return errors


def check_code_constants(repo_root: Path, expected: set[str]) -> list[str]:
    errors: list[str] = []
    for rel, constant in (
        ("scripts/validate_ara.py", "ALLOWED_PROOF_CLASSIFICATIONS"),
        ("scripts/plot_trace2skill_ara.py", "ALLOWED_PROOF_CLASSIFICATIONS"),
        ("scripts/import_trace2skill_eval_results.py", "ALLOWED_PROOF_CLASSIFICATIONS"),
    ):
        actual = extract_python_set(repo_root / rel, constant)
        if actual != expected:
            errors.append(f"{rel} {constant} is {sorted(actual)}, expected {sorted(expected)}")

    result_intake = repo_root / "scripts/check_trace2skill_result_intake.py"
    non_overlay = extract_python_set(result_intake, "NON_OVERLAY_ONLY_CLASSIFICATIONS")
    paper_denominator = extract_python_set(result_intake, "PAPER_DENOMINATOR_CLASSIFICATIONS")
    if not {"model-one-case", "evolving-split-run", "training-validation-candidate"}.issubset(non_overlay):
        errors.append("check_trace2skill_result_intake.py must keep model/evolving/training labels non-overlay-only")
    if not {"held-out-single-seed-candidate", "seed-aggregate-candidate"}.issubset(paper_denominator):
        errors.append("check_trace2skill_result_intake.py must treat held-out and seed aggregate labels as paper-denominator-like")
    return errors


def check_runbook_labels(repo_root: Path, ara_root: Path) -> list[str]:
    errors: list[str] = []
    stages = load_runbook_stages(ara_root)
    runbook_labels = {
        stage["allowed_label"] for stage in stages if isinstance(stage.get("allowed_label"), str)
    }
    unexpected = runbook_labels - RESULT_PROOF_CLASSIFICATIONS - RUNBOOK_ONLY_LABELS
    if unexpected:
        errors.append(f"full_denominator_runbook.json uses unsupported allowed_label values: {sorted(unexpected)}")
    missing_from_runbook = {
        "deterministic-one-case",
        "model-one-case",
        "paper-subset",
        "evolving-split-run",
        "training-validation-candidate",
        "held-out-single-seed-candidate",
        "seed-aggregate-candidate",
        "paper-denominator-reproduction",
    } - runbook_labels
    if missing_from_runbook:
        errors.append(f"full_denominator_runbook.json missing staged labels: {sorted(missing_from_runbook)}")

    stages_by_id = {stage.get("id"): stage for stage in stages}
    for stage_id, expected_forbidden in EXPECTED_FORBIDDEN_LABELS.items():
        stage = stages_by_id.get(stage_id)
        if not isinstance(stage, dict):
            errors.append(f"full_denominator_runbook.json missing stage {stage_id}")
            continue
        actual_forbidden = stage.get("forbidden_label")
        if actual_forbidden != expected_forbidden:
            errors.append(
                f"full_denominator_runbook.json stage {stage_id} forbidden_label is "
                f"{actual_forbidden!r}, expected {expected_forbidden!r}"
            )
    forbidden_labels = {
        stage["forbidden_label"] for stage in stages if isinstance(stage.get("forbidden_label"), str)
    }
    leaked = forbidden_labels & runbook_labels
    if leaked:
        errors.append(
            f"full_denominator_runbook.json uses forbidden labels as allowed labels: {sorted(leaked)}"
        )

    errors.extend(check_readme_labels(ara_root, RESULT_PROOF_CLASSIFICATIONS))
    errors.extend(check_schema_notes(ara_root))
    errors.extend(check_plan_labels(ara_root, RESULT_PROOF_CLASSIFICATIONS))
    errors.extend(check_code_constants(repo_root, RESULT_PROOF_CLASSIFICATIONS))
    return errors


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("ara_dir", type=Path)
    args = parser.parse_args()
    ara_root = args.ara_dir.resolve()
    repo_root = repo_root_for(ara_root)

    errors = check_runbook_labels(repo_root, ara_root)
    if errors:
        for error in errors:
            print(f"FAIL: {error}")
        return 1
    print(f"PASS: {args.ara_dir} runbook labels")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
