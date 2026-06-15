#!/usr/bin/env python3
"""Seal Level 1 structural validator for local ARA packages."""

from __future__ import annotations

import argparse
import importlib.util
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
    "model-one-case",
    "paper-subset",
    "evolving-split-run",
    "training-validation-candidate",
    "held-out-single-seed-candidate",
    "seed-aggregate-candidate",
    "paper-denominator-candidate",
    "paper-denominator-reproduction",
}
ALLOWED_METRIC_UNITS = {"percent", "delta_points", "minutes", "fraction"}
SUPPORTED_RESULT_PANELS = {
    "same_model_deepening_vrf",
    "avg_improvement",
    "parallel_vs_sequential",
    "reasoningbank",
}
REQUIRED_TRACE2SKILL_MECHANICS_TESTS = [
    "manifest",
    "run_artifacts",
    "patch_bridge",
    "patch_replay",
    "one_case",
    "one_case_run",
    "cli",
    "workbook_score",
    "acp_external_worker",
]


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


def repo_root_for(root: Path) -> Path:
    for candidate in (root, *root.parents):
        if (candidate / "examples/trace2skill_spreadsheetbench/tests").is_dir():
            return candidate
    return Path.cwd()


def validate_trace2skill_table_fidelity(errors: list[str], root: Path) -> None:
    repo_root = repo_root_for(root.resolve())
    checker_path = repo_root / "scripts/check_trace2skill_table_fidelity.py"
    if not checker_path.is_file():
        fail(errors, "missing scripts/check_trace2skill_table_fidelity.py")
        return
    spec = importlib.util.spec_from_file_location("check_trace2skill_table_fidelity", checker_path)
    if spec is None or spec.loader is None:
        fail(errors, f"cannot import {checker_path}")
        return
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    for table_error in module.check_table_fidelity(repo_root, root):
        fail(errors, f"table fidelity: {table_error}")


def validate_trace2skill_prompt_index(errors: list[str], root: Path) -> None:
    repo_root = repo_root_for(root.resolve())
    checker_path = repo_root / "scripts/check_trace2skill_prompt_index.py"
    if not checker_path.is_file():
        fail(errors, "missing scripts/check_trace2skill_prompt_index.py")
        return
    spec = importlib.util.spec_from_file_location("check_trace2skill_prompt_index", checker_path)
    if spec is None or spec.loader is None:
        fail(errors, f"cannot import {checker_path}")
        return
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    for prompt_error in module.check_prompt_index(repo_root, root):
        fail(errors, f"prompt index: {prompt_error}")


def validate_trace2skill_prompt_manifest(errors: list[str], root: Path) -> None:
    repo_root = repo_root_for(root.resolve())
    checker_path = repo_root / "scripts/check_trace2skill_prompt_manifest.py"
    if not checker_path.is_file():
        fail(errors, "missing scripts/check_trace2skill_prompt_manifest.py")
        return
    spec = importlib.util.spec_from_file_location("check_trace2skill_prompt_manifest", checker_path)
    if spec is None or spec.loader is None:
        fail(errors, f"cannot import {checker_path}")
        return
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    for prompt_error in module.check_prompt_manifest(repo_root, root):
        fail(errors, f"prompt manifest: {prompt_error}")


def validate_trace2skill_figure_index(errors: list[str], root: Path) -> None:
    repo_root = repo_root_for(root.resolve())
    checker_path = repo_root / "scripts/check_trace2skill_figure_index.py"
    if not checker_path.is_file():
        fail(errors, "missing scripts/check_trace2skill_figure_index.py")
        return
    spec = importlib.util.spec_from_file_location("check_trace2skill_figure_index", checker_path)
    if spec is None or spec.loader is None:
        fail(errors, f"cannot import {checker_path}")
        return
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    for figure_error in module.check_figure_index(repo_root, root):
        fail(errors, f"figure index: {figure_error}")


def validate_trace2skill_config_fidelity(errors: list[str], root: Path) -> None:
    repo_root = repo_root_for(root.resolve())
    checker_path = repo_root / "scripts/check_trace2skill_config_fidelity.py"
    if not checker_path.is_file():
        fail(errors, "missing scripts/check_trace2skill_config_fidelity.py")
        return
    spec = importlib.util.spec_from_file_location("check_trace2skill_config_fidelity", checker_path)
    if spec is None or spec.loader is None:
        fail(errors, f"cannot import {checker_path}")
        return
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    for config_error in module.check_config_fidelity(repo_root, root):
        fail(errors, f"config fidelity: {config_error}")


def validate_trace2skill_dataset_manifest_freshness(errors: list[str], root: Path) -> None:
    repo_root = repo_root_for(root.resolve())
    checker_path = repo_root / "scripts/check_trace2skill_dataset_manifest_freshness.py"
    if not checker_path.is_file():
        fail(errors, "missing scripts/check_trace2skill_dataset_manifest_freshness.py")
        return
    spec = importlib.util.spec_from_file_location("check_trace2skill_dataset_manifest_freshness", checker_path)
    if spec is None or spec.loader is None:
        fail(errors, f"cannot import {checker_path}")
        return
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    for freshness_error in module.check_dataset_manifest_freshness(repo_root, root.resolve()):
        fail(errors, f"dataset manifest freshness: {freshness_error}")


def validate_trace2skill_upstream_code_manifest(errors: list[str], root: Path) -> None:
    repo_root = repo_root_for(root.resolve())
    checker_path = repo_root / "scripts/check_trace2skill_upstream_code_manifest.py"
    if not checker_path.is_file():
        fail(errors, "missing scripts/check_trace2skill_upstream_code_manifest.py")
        return
    spec = importlib.util.spec_from_file_location("check_trace2skill_upstream_code_manifest", checker_path)
    if spec is None or spec.loader is None:
        fail(errors, f"cannot import {checker_path}")
        return
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    for code_error in module.check_upstream_code_manifest(repo_root, root):
        fail(errors, f"upstream code manifest: {code_error}")


def validate_trace2skill_one_case_artifacts(errors: list[str], root: Path) -> None:
    repo_root = repo_root_for(root.resolve())
    checker_path = repo_root / "scripts/check_trace2skill_one_case_artifacts.py"
    if not checker_path.is_file():
        fail(errors, "missing scripts/check_trace2skill_one_case_artifacts.py")
        return
    spec = importlib.util.spec_from_file_location("check_trace2skill_one_case_artifacts", checker_path)
    if spec is None or spec.loader is None:
        fail(errors, f"cannot import {checker_path}")
        return
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    for one_case_error in module.check_one_case_artifacts(repo_root, root):
        fail(errors, f"one-case artifacts: {one_case_error}")


def validate_trace2skill_one_case_result_freshness(errors: list[str], root: Path) -> None:
    repo_root = repo_root_for(root.resolve())
    checker_path = repo_root / "scripts/check_trace2skill_one_case_result_freshness.py"
    if not checker_path.is_file():
        fail(errors, "missing scripts/check_trace2skill_one_case_result_freshness.py")
        return
    spec = importlib.util.spec_from_file_location("check_trace2skill_one_case_result_freshness", checker_path)
    if spec is None or spec.loader is None:
        fail(errors, f"cannot import {checker_path}")
        return
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    for freshness_error in module.check_one_case_result_freshness(repo_root, root.resolve()):
        fail(errors, f"one-case result freshness: {freshness_error}")


def validate_trace2skill_stage2_prompt_artifacts(errors: list[str], root: Path) -> None:
    repo_root = repo_root_for(root.resolve())
    checker_path = repo_root / "scripts/check_trace2skill_stage2_prompt_artifacts.py"
    if not checker_path.is_file():
        fail(errors, "missing scripts/check_trace2skill_stage2_prompt_artifacts.py")
        return
    spec = importlib.util.spec_from_file_location("check_trace2skill_stage2_prompt_artifacts", checker_path)
    if spec is None or spec.loader is None:
        fail(errors, f"cannot import {checker_path}")
        return
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    for prompt_error in module.check_stage2_prompt_artifacts(repo_root, root.resolve()):
        fail(errors, f"Stage 2 rendered prompt artifacts: {prompt_error}")


def validate_trace2skill_plot_provenance(errors: list[str], root: Path) -> None:
    repo_root = repo_root_for(root.resolve())
    checker_path = repo_root / "scripts/check_trace2skill_plot_provenance.py"
    if not checker_path.is_file():
        fail(errors, "missing scripts/check_trace2skill_plot_provenance.py")
        return
    spec = importlib.util.spec_from_file_location("check_trace2skill_plot_provenance", checker_path)
    if spec is None or spec.loader is None:
        fail(errors, f"cannot import {checker_path}")
        return
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    for plot_error in module.check_plot_provenance(root):
        fail(errors, f"plot provenance: {plot_error}")


def validate_trace2skill_plot_freshness(errors: list[str], root: Path) -> None:
    repo_root = repo_root_for(root.resolve())
    checker_path = repo_root / "scripts/check_trace2skill_plot_freshness.py"
    if not checker_path.is_file():
        fail(errors, "missing scripts/check_trace2skill_plot_freshness.py")
        return
    spec = importlib.util.spec_from_file_location("check_trace2skill_plot_freshness", checker_path)
    if spec is None or spec.loader is None:
        fail(errors, f"cannot import {checker_path}")
        return
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    for freshness_error in module.check_plot_freshness(repo_root, root.resolve()):
        fail(errors, f"plot freshness: {freshness_error}")


def validate_trace2skill_plot_result_intake(errors: list[str], root: Path) -> None:
    repo_root = repo_root_for(root.resolve())
    checker_path = repo_root / "scripts/check_trace2skill_plot_result_intake.py"
    if not checker_path.is_file():
        fail(errors, "missing scripts/check_trace2skill_plot_result_intake.py")
        return
    spec = importlib.util.spec_from_file_location("check_trace2skill_plot_result_intake", checker_path)
    if spec is None or spec.loader is None:
        fail(errors, f"cannot import {checker_path}")
        return
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    for plot_error in module.check_plot_result_intake(repo_root, root.resolve()):
        fail(errors, f"plot result-intake gate: {plot_error}")


def validate_trace2skill_result_intake(errors: list[str], root: Path) -> None:
    repo_root = repo_root_for(root.resolve())
    checker_path = repo_root / "scripts/check_trace2skill_result_intake.py"
    if not checker_path.is_file():
        fail(errors, "missing scripts/check_trace2skill_result_intake.py")
        return
    spec = importlib.util.spec_from_file_location("check_trace2skill_result_intake", checker_path)
    if spec is None or spec.loader is None:
        fail(errors, f"cannot import {checker_path}")
        return
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    for result_error in module.check_result_intake(repo_root, root):
        fail(errors, f"result intake: {result_error}")


def validate_trace2skill_importer_fixture(errors: list[str], root: Path) -> None:
    repo_root = repo_root_for(root.resolve())
    checker_path = repo_root / "scripts/check_trace2skill_importer_fixture.py"
    if not checker_path.is_file():
        fail(errors, "missing scripts/check_trace2skill_importer_fixture.py")
        return
    spec = importlib.util.spec_from_file_location("check_trace2skill_importer_fixture", checker_path)
    if spec is None or spec.loader is None:
        fail(errors, f"cannot import {checker_path}")
        return
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    for fixture_error in module.check_importer_fixture(repo_root, root.resolve()):
        fail(errors, f"official-eval importer fixture: {fixture_error}")


def validate_trace2skill_evidence_bindings(errors: list[str], root: Path) -> None:
    repo_root = repo_root_for(root.resolve())
    checker_path = repo_root / "scripts/check_trace2skill_evidence_bindings.py"
    if not checker_path.is_file():
        fail(errors, "missing scripts/check_trace2skill_evidence_bindings.py")
        return
    spec = importlib.util.spec_from_file_location("check_trace2skill_evidence_bindings", checker_path)
    if spec is None or spec.loader is None:
        fail(errors, f"cannot import {checker_path}")
        return
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    for binding_error in module.check_evidence_bindings(repo_root, root):
        fail(errors, f"evidence bindings: {binding_error}")


def validate_trace2skill_status_docs(errors: list[str], root: Path) -> None:
    repo_root = repo_root_for(root.resolve())
    checker_path = repo_root / "scripts/check_trace2skill_status_docs.py"
    if not checker_path.is_file():
        fail(errors, "missing scripts/check_trace2skill_status_docs.py")
        return
    spec = importlib.util.spec_from_file_location("check_trace2skill_status_docs", checker_path)
    if spec is None or spec.loader is None:
        fail(errors, f"cannot import {checker_path}")
        return
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    for status_error in module.check_status_docs(repo_root, root):
        fail(errors, f"status docs: {status_error}")


def validate_trace2skill_rigor_followup(errors: list[str], root: Path) -> None:
    repo_root = repo_root_for(root.resolve())
    checker_path = repo_root / "scripts/check_trace2skill_rigor_followup.py"
    if not checker_path.is_file():
        fail(errors, "missing scripts/check_trace2skill_rigor_followup.py")
        return
    spec = importlib.util.spec_from_file_location("check_trace2skill_rigor_followup", checker_path)
    if spec is None or spec.loader is None:
        fail(errors, f"cannot import {checker_path}")
        return
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    for rigor_error in module.check_rigor_followup(repo_root, root):
        fail(errors, f"rigor follow-up: {rigor_error}")


def validate_trace2skill_runbook_labels(errors: list[str], root: Path) -> None:
    repo_root = repo_root_for(root.resolve())
    checker_path = repo_root / "scripts/check_trace2skill_runbook_labels.py"
    if not checker_path.is_file():
        fail(errors, "missing scripts/check_trace2skill_runbook_labels.py")
        return
    spec = importlib.util.spec_from_file_location("check_trace2skill_runbook_labels", checker_path)
    if spec is None or spec.loader is None:
        fail(errors, f"cannot import {checker_path}")
        return
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    for label_error in module.check_runbook_labels(repo_root, root):
        fail(errors, f"runbook labels: {label_error}")


def validate_trace2skill_approval_state(errors: list[str], root: Path) -> None:
    repo_root = repo_root_for(root.resolve())
    checker_path = repo_root / "scripts/check_trace2skill_approval_state.py"
    if not checker_path.is_file():
        fail(errors, "missing scripts/check_trace2skill_approval_state.py")
        return
    spec = importlib.util.spec_from_file_location("check_trace2skill_approval_state", checker_path)
    if spec is None or spec.loader is None:
        fail(errors, f"cannot import {checker_path}")
        return
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    for approval_error in module.check_approval_state(repo_root, root.resolve()):
        fail(errors, f"approval state: {approval_error}")


def validate_trace2skill_runbook_freshness(errors: list[str], root: Path) -> None:
    repo_root = repo_root_for(root.resolve())
    checker_path = repo_root / "scripts/check_trace2skill_runbook_freshness.py"
    if not checker_path.is_file():
        fail(errors, "missing scripts/check_trace2skill_runbook_freshness.py")
        return
    spec = importlib.util.spec_from_file_location("check_trace2skill_runbook_freshness", checker_path)
    if spec is None or spec.loader is None:
        fail(errors, f"cannot import {checker_path}")
        return
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    for freshness_error in module.check_runbook_freshness(repo_root, root):
        fail(errors, f"runbook freshness: {freshness_error}")


def validate_trace2skill_artifact_contract(errors: list[str], root: Path) -> None:
    repo_root = repo_root_for(root.resolve())
    checker_path = repo_root / "scripts/check_trace2skill_artifact_contract.py"
    if not checker_path.is_file():
        fail(errors, "missing scripts/check_trace2skill_artifact_contract.py")
        return
    spec = importlib.util.spec_from_file_location("check_trace2skill_artifact_contract", checker_path)
    if spec is None or spec.loader is None:
        fail(errors, f"cannot import {checker_path}")
        return
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    for contract_error in module.check_artifact_contract(repo_root, root):
        fail(errors, f"artifact contract: {contract_error}")


def validate_trace2skill_closeout_freshness(errors: list[str], root: Path) -> None:
    repo_root = repo_root_for(root.resolve())
    checker_path = repo_root / "scripts/check_trace2skill_closeout_freshness.py"
    if not checker_path.is_file():
        fail(errors, "missing scripts/check_trace2skill_closeout_freshness.py")
        return
    spec = importlib.util.spec_from_file_location("check_trace2skill_closeout_freshness", checker_path)
    if spec is None or spec.loader is None:
        fail(errors, f"cannot import {checker_path}")
        return
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    for freshness_error in module.check_closeout_freshness(repo_root, root.resolve()):
        fail(errors, f"closeout freshness: {freshness_error}")


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
    for field in ("cost", "runtime"):
        if not isinstance(record[field], dict):
            fail(errors, f"{prefix} {field} must be an object")

    binding = record["plot_binding"]
    if binding is not None and not isinstance(binding, dict):
        fail(errors, f"{prefix} plot_binding must be an object or null")
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

    validate_trace2skill_table_fidelity(errors, root)
    validate_trace2skill_prompt_index(errors, root)
    validate_trace2skill_prompt_manifest(errors, root)
    validate_trace2skill_figure_index(errors, root)
    validate_trace2skill_config_fidelity(errors, root)
    validate_trace2skill_dataset_manifest_freshness(errors, root)
    validate_trace2skill_upstream_code_manifest(errors, root)
    validate_trace2skill_one_case_artifacts(errors, root)
    validate_trace2skill_one_case_result_freshness(errors, root)
    validate_trace2skill_stage2_prompt_artifacts(errors, root)
    validate_trace2skill_plot_provenance(errors, root)
    validate_trace2skill_plot_freshness(errors, root)
    validate_trace2skill_plot_result_intake(errors, root)
    validate_trace2skill_result_intake(errors, root)
    validate_trace2skill_importer_fixture(errors, root)
    validate_trace2skill_evidence_bindings(errors, root)
    validate_trace2skill_status_docs(errors, root)
    validate_trace2skill_rigor_followup(errors, root)
    validate_trace2skill_runbook_labels(errors, root)
    validate_trace2skill_approval_state(errors, root)
    validate_trace2skill_runbook_freshness(errors, root)
    validate_trace2skill_artifact_contract(errors, root)
    validate_trace2skill_closeout_freshness(errors, root)

    mechanics_evidence = root / "evidence/leaven_mechanics_tests.md"
    if mechanics_evidence.is_file():
        mechanics_text = read(mechanics_evidence)
        repo_root = repo_root_for(root.resolve())
        for test_name in REQUIRED_TRACE2SKILL_MECHANICS_TESTS:
            test_path = repo_root / f"examples/trace2skill_spreadsheetbench/tests/{test_name}.rs"
            if not test_path.is_file():
                fail(errors, f"missing Trace2Skill mechanics test file: {test_path.relative_to(repo_root)}")
            expected_command = f"cargo test -p trace2skill_spreadsheetbench --test {test_name}"
            if expected_command not in mechanics_text:
                fail(errors, f"leaven_mechanics_tests.md missing classified target: {expected_command}")
    else:
        fail(errors, "missing evidence/leaven_mechanics_tests.md")

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
                    summary = closeout_audit.get("result_record_summary")
                    if summary is not None:
                        if not isinstance(summary, dict):
                            fail(errors, "results/closeout_audit.json result_record_summary must be an object")
                        elif summary.get("paper_denominator_records") not in {0, None}:
                            fail(errors, "results/closeout_audit.json must report zero paper_denominator_records until paper denominator is proven")
                    intake_summary = closeout_audit.get("result_intake_summary")
                    if not isinstance(intake_summary, dict):
                        fail(errors, "results/closeout_audit.json missing result_intake_summary object")
                    else:
                        if intake_summary.get("valid") is not True:
                            fail(errors, "results/closeout_audit.json result_intake_summary.valid must be true")
                        if intake_summary.get("checker") != "scripts/check_trace2skill_result_intake.py":
                            fail(errors, "results/closeout_audit.json result_intake_summary.checker must name result-intake checker")
                        if intake_summary.get("errors") != []:
                            fail(errors, "results/closeout_audit.json result_intake_summary.errors must be empty")
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

            runbook_path = results_dir / "full_denominator_runbook.json"
            if runbook_path.exists():
                try:
                    runbook = json.loads(read(runbook_path))
                except json.JSONDecodeError as exc:
                    fail(errors, f"results/full_denominator_runbook.json is not valid JSON: {exc}")
                    runbook = None
                if isinstance(runbook, dict):
                    if runbook.get("schema_version") != "leaven.trace2skill.runbook.v1":
                        fail(errors, "results/full_denominator_runbook.json has wrong schema_version")
                    approval_state = runbook.get("approval_state")
                    if not isinstance(approval_state, dict):
                        fail(errors, "results/full_denominator_runbook.json missing approval_state")
                    elif approval_state.get("normal_preflight_passes") is not False:
                        fail(errors, "results/full_denominator_runbook.json must keep normal_preflight_passes false until approved")
                    stages = runbook.get("stages")
                    if not isinstance(stages, list):
                        fail(errors, "results/full_denominator_runbook.json missing stages")
                    else:
                        stage_ids = {stage.get("id") for stage in stages if isinstance(stage, dict)}
                        for stage_id in ("G0", "G1", "G1M", "G2", "G3", "G3V", "G4", "G5", "G6"):
                            if stage_id not in stage_ids:
                                fail(errors, f"results/full_denominator_runbook.json missing {stage_id}")
                        by_id = {stage.get("id"): stage for stage in stages if isinstance(stage, dict)}
                        expected_dataset_kinds = {
                            "G0": None,
                            "G1": "one-case",
                            "G1M": "one-case",
                            "G2": "held-out-subset",
                            "G3": "exact-range",
                            "G3V": "exact-range",
                            "G4": "exact-range",
                            "G5": "aggregate",
                            "G6": "full-paper",
                        }
                        for stage_id, expected_kind in expected_dataset_kinds.items():
                            stage = by_id.get(stage_id)
                            if stage is None:
                                continue
                            expected_slice = stage.get("expected_dataset_slice")
                            if expected_kind is None:
                                if expected_slice is not None:
                                    fail(errors, f"results/full_denominator_runbook.json {stage_id} expected_dataset_slice must be null")
                            elif not isinstance(expected_slice, dict):
                                fail(errors, f"results/full_denominator_runbook.json {stage_id} missing expected_dataset_slice")
                            elif expected_slice.get("kind") != expected_kind:
                                fail(errors, f"results/full_denominator_runbook.json {stage_id} expected_dataset_slice kind must be {expected_kind}")
                        expected_seed_kinds = {
                            "G0": None,
                            "G1": None,
                            "G1M": "exact",
                            "G2": "exact",
                            "G3": "one-of",
                            "G3V": "one-of",
                            "G4": "one-of",
                            "G5": "all-of",
                            "G6": "all-of",
                        }
                        for stage_id, expected_kind in expected_seed_kinds.items():
                            stage = by_id.get(stage_id)
                            if stage is None:
                                continue
                            seed_policy = stage.get("expected_seed_policy")
                            if expected_kind is None:
                                if seed_policy is not None:
                                    fail(errors, f"results/full_denominator_runbook.json {stage_id} expected_seed_policy must be null")
                            elif not isinstance(seed_policy, dict):
                                fail(errors, f"results/full_denominator_runbook.json {stage_id} missing expected_seed_policy")
                            elif seed_policy.get("kind") != expected_kind:
                                fail(errors, f"results/full_denominator_runbook.json {stage_id} expected_seed_policy kind must be {expected_kind}")
                            elif expected_kind == "exact" and seed_policy.get("seed") != 41:
                                fail(errors, f"results/full_denominator_runbook.json {stage_id} expected_seed_policy seed must be 41")
                            elif expected_kind in {"one-of", "all-of"} and seed_policy.get("seeds") != [41, 42, 43]:
                                fail(errors, f"results/full_denominator_runbook.json {stage_id} expected_seed_policy seeds must be [41, 42, 43]")
                        expected_runtime_kinds = {
                            "G0": None,
                            "G1": None,
                            "G1M": "upstream-run",
                            "G2": "upstream-run",
                            "G3": "skill-evolution",
                            "G3V": "upstream-run",
                            "G4": "upstream-run",
                            "G5": None,
                            "G6": None,
                        }
                        for stage_id, expected_kind in expected_runtime_kinds.items():
                            stage = by_id.get(stage_id)
                            if stage is None:
                                continue
                            runtime_policy = stage.get("expected_runtime_policy")
                            if expected_kind is None:
                                if runtime_policy is not None:
                                    fail(errors, f"results/full_denominator_runbook.json {stage_id} expected_runtime_policy must be null")
                            elif not isinstance(runtime_policy, dict):
                                fail(errors, f"results/full_denominator_runbook.json {stage_id} missing expected_runtime_policy")
                            elif runtime_policy.get("kind") != expected_kind:
                                fail(errors, f"results/full_denominator_runbook.json {stage_id} expected_runtime_policy kind must be {expected_kind}")
                            else:
                                expected_workers = 1 if stage_id == "G1M" else 128
                                if runtime_policy.get("workers") != expected_workers:
                                    fail(errors, f"results/full_denominator_runbook.json {stage_id} expected_runtime_policy workers must be {expected_workers}")
                                if runtime_policy.get("max_turns") != 100:
                                    fail(errors, f"results/full_denominator_runbook.json {stage_id} expected_runtime_policy max_turns must be 100")
                                if expected_kind == "skill-evolution" and runtime_policy.get("merge_batch_size") != 32:
                                    fail(errors, f"results/full_denominator_runbook.json {stage_id} expected_runtime_policy merge_batch_size must be 32")
                        expected_command_kinds = {
                            "G0": None,
                            "G1": None,
                            "G1M": "upstream-eval",
                            "G2": "upstream-eval",
                            "G3": "skill-evolution",
                            "G3V": "upstream-eval",
                            "G4": "upstream-eval",
                            "G5": None,
                            "G6": None,
                        }
                        for stage_id, expected_kind in expected_command_kinds.items():
                            stage = by_id.get(stage_id)
                            if stage is None:
                                continue
                            command_policy = stage.get("expected_command_policy")
                            if expected_kind is None:
                                if command_policy is not None:
                                    fail(errors, f"results/full_denominator_runbook.json {stage_id} expected_command_policy must be null")
                            elif not isinstance(command_policy, dict):
                                fail(errors, f"results/full_denominator_runbook.json {stage_id} missing expected_command_policy")
                            elif command_policy.get("kind") != expected_kind:
                                fail(errors, f"results/full_denominator_runbook.json {stage_id} expected_command_policy kind must be {expected_kind}")
                            else:
                                fragments = command_policy.get("required_source_command_fragments")
                                if not isinstance(fragments, list):
                                    fail(errors, f"results/full_denominator_runbook.json {stage_id} expected_command_policy fragments must be a list")
                                    continue
                                required = ["run_spreadsheetbench.py", "evaluate_with_official.py"]
                                if expected_kind == "skill-evolution":
                                    required.extend(
                                        [
                                            "analyze_results.py",
                                            "analysis/run_error_analysis.py",
                                            "analysis/run_success_analysis_llm.py",
                                            "skill_evolver.run_parallel_skill_evolution",
                                        ]
                                    )
                                for fragment in required:
                                    if fragment not in fragments:
                                        fail(
                                            errors,
                                            f"results/full_denominator_runbook.json {stage_id} expected_command_policy missing {fragment}",
                                        )
                        expected_aggregate_kinds = {
                            "G0": None,
                            "G1": None,
                            "G1M": None,
                            "G2": None,
                            "G3": None,
                            "G3V": None,
                            "G4": None,
                            "G5": "seed-aggregate",
                            "G6": "full-paper",
                        }
                        for stage_id, expected_kind in expected_aggregate_kinds.items():
                            stage = by_id.get(stage_id)
                            if stage is None:
                                continue
                            aggregate_policy = stage.get("expected_aggregate_policy")
                            if expected_kind is None:
                                if aggregate_policy is not None:
                                    fail(errors, f"results/full_denominator_runbook.json {stage_id} expected_aggregate_policy must be null")
                            elif not isinstance(aggregate_policy, dict):
                                fail(errors, f"results/full_denominator_runbook.json {stage_id} missing expected_aggregate_policy")
                            elif aggregate_policy.get("kind") != expected_kind:
                                fail(errors, f"results/full_denominator_runbook.json {stage_id} expected_aggregate_policy kind must be {expected_kind}")
                            elif expected_kind == "seed-aggregate":
                                expected_policy = {
                                    "kind": "seed-aggregate",
                                    "source_runbook_stage_id": "G4",
                                    "source_proof_classification": "held-out-single-seed-candidate",
                                    "required_seeds": [41, 42, 43],
                                    "source_result_paths_min": 3,
                                }
                                if aggregate_policy != expected_policy:
                                    fail(
                                        errors,
                                        "results/full_denominator_runbook.json G5 aggregate source policy must match the held-out seed aggregate contract",
                                    )
                            elif expected_kind == "full-paper":
                                expected_policy = {
                                    "kind": "full-paper",
                                    "source_proof_classifications": [
                                        "training-validation-candidate",
                                        "seed-aggregate-candidate",
                                    ],
                                    "source_result_paths_min": 1,
                                }
                                if aggregate_policy != expected_policy:
                                    fail(
                                        errors,
                                        "results/full_denominator_runbook.json G6 aggregate source policy must be training-validation-candidate plus seed-aggregate-candidate only",
                                    )

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
