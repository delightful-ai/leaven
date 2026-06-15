#!/usr/bin/env python3
"""Audit Trace2Skill ARA closeout status against the goal handoff."""

from __future__ import annotations

import argparse
import importlib.util
import json
import sys
from pathlib import Path
from typing import Any


ACCEPTANCE_IDS = [
    "ara_level1_valid",
    "plots_from_ara",
    "current_mechanics_classified",
    "one_case_live_or_explicit_blocker",
    "full_denominator_plan_approved",
    "reproduced_claim_limited_to_actual_denominator",
]

FORBIDDEN_PROXY_LABELS = [
    "historical-yaml",
    "ara-shape-only",
    "paper-target-plot",
    "trace2skill-tiny-live",
    "one-case-only",
    "mechanics-tests",
    "harbor-adapter",
    "subset-improvement",
]


def import_approval_checker(repo_root: Path) -> Any:
    path = repo_root / "scripts/check_trace2skill_approval_packet.py"
    spec = importlib.util.spec_from_file_location("check_trace2skill_approval_packet", path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot import {path}")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def rel(path: Path, root: Path) -> str:
    return path.relative_to(root).as_posix()


def file_exists(repo_root: Path, rel_path: str) -> bool:
    return (repo_root / rel_path).is_file()


def json_file(repo_root: Path, rel_path: str) -> dict[str, Any]:
    path = repo_root / rel_path
    return json.loads(path.read_text(encoding="utf-8"))


def jsonl_records(path: Path) -> list[dict[str, Any]]:
    records: list[dict[str, Any]] = []
    for line in path.read_text(encoding="utf-8").splitlines():
        if line.strip():
            records.append(json.loads(line))
    return records


def status_entry(status: str, evidence: list[str], remaining: list[str]) -> dict[str, Any]:
    return {
        "status": status,
        "evidence": evidence,
        "remaining": remaining,
    }


def audit(repo_root: Path, ara_dir: Path) -> dict[str, Any]:
    ara_rel = rel(ara_dir, repo_root)
    results_dir = ara_dir / "results"
    result_jsonl = sorted(results_dir.glob("*.jsonl")) if results_dir.is_dir() else []
    result_records = [record for path in result_jsonl for record in jsonl_records(path)]
    paper_denominator_result_records = [
        record
        for record in result_records
        if record.get("proof_classification")
        in {"paper-denominator-candidate", "paper-denominator-reproduction"}
    ]

    dataset_manifest_path = "docs/ara/trace2skill_spreadsheetbench/results/dataset_manifest.json"
    dataset_manifest = json_file(repo_root, dataset_manifest_path)
    score_report_path = "tmp/trace2skill-one-case-live/score_report.json"
    score_report = json_file(repo_root, score_report_path) if file_exists(repo_root, score_report_path) else {}

    approval = import_approval_checker(repo_root)
    full_run_plan = ara_dir / "results/full_run_plan.md"
    packet = approval.approval_packet(full_run_plan.read_text(encoding="utf-8"))
    approval_errors = approval.packet_errors(packet)

    acceptance: dict[str, dict[str, Any]] = {}
    acceptance["ara_level1_valid"] = status_entry(
        "satisfied_current_package",
        [
            f"{ara_rel}/PAPER.md",
            f"{ara_rel}/logic/claims.md",
            f"{ara_rel}/logic/experiments.md",
            f"{ara_rel}/evidence/tables",
            f"{ara_rel}/validation.md",
            "uv run --with pyyaml python scripts/validate_ara.py docs/ara/trace2skill_spreadsheetbench",
        ],
        ["Re-run the validator after any ARA evidence or schema change."],
    )
    acceptance["plots_from_ara"] = status_entry(
        "satisfied_targets_only",
        [
            f"{ara_rel}/plots/trace2skill_targets.png",
            "scripts/plot_trace2skill_ara.py",
            f"{ara_rel}/results/leaven_result_schema.md",
            f"{ara_rel}/results/deterministic_one_case.jsonl",
        ],
        [
            "Only non-overlay deterministic one-case JSONL exists; no paper-denominator overlay rows exist yet.",
            "Target plots remain target evidence, not reproduction evidence.",
        ],
    )
    acceptance["current_mechanics_classified"] = status_entry(
        "satisfied_current_tests",
        [
            f"{ara_rel}/evidence/leaven_mechanics_tests.md",
            f"{ara_rel}/validation.md",
        ],
        ["Re-run focused Rust tests after changing example mechanics or proof classifications."],
    )

    one_case_ok = bool(score_report.get("passed")) and file_exists(
        repo_root, "tmp/trace2skill-one-case-live/13-1_output.xlsx"
    )
    acceptance["one_case_live_or_explicit_blocker"] = status_entry(
        "satisfied_deterministic_one_case" if one_case_ok else "blocked_or_missing",
        [
            f"{ara_rel}/results/one_case_live.md",
            f"{ara_rel}/results/deterministic_one_case.jsonl",
            "tmp/trace2skill-one-case-live/13-1_output.xlsx",
            score_report_path,
            "tmp/trace2skill-one-case-live/trajectory.json",
            "tmp/trace2skill-one-case-live/acp_result.json",
        ],
        [
            "This is deterministic local ACP one-case evidence only.",
            "Model-backed one-case evidence remains absent until approved.",
        ],
    )

    acceptance["full_denominator_plan_approved"] = status_entry(
        "blocked",
        [
            f"{ara_rel}/results/full_run_plan.md",
            f"{ara_rel}/results/full_denominator_runbook.md",
            f"{ara_rel}/results/full_denominator_runbook.json",
            dataset_manifest_path,
            "scripts/check_trace2skill_approval_packet.py",
        ],
        approval_errors,
    )
    acceptance["reproduced_claim_limited_to_actual_denominator"] = status_entry(
        "guardrail_active_not_final_closeout",
        [
            f"{ara_rel}/results/denominator_status.md",
            f"{ara_rel}/reviews/rigor_review.md",
            f"{ara_rel}/level2_report.json",
        ],
        [
            "No held-out 200..400 paper-denominator result rows exist.",
            "No seed aggregate rows exist.",
            "No cross-model paper-denominator rows exist.",
            "Final closeout remains impossible while normal approval preflight fails.",
        ],
    )

    reproduced_denominators = [
        "paper-targets-captured",
        "mechanics-tests-classified",
        "deterministic-one-case-13-1",
    ]
    missing_denominators = [
        "model-backed-one-case-13-1",
        "small-N-paper-subset",
        "evolving-split-0..200",
        "held-out-split-200..400",
        "seed-aggregate-41-42-43",
        "cross-model-paper-rows",
        "full-paper-denominator",
    ]

    return {
        "schema_version": "leaven.trace2skill.closeout_audit.v1",
        "overall_complete": False,
        "reason": "Full paper-denominator reproduction is not proven: approval preflight is blocked and no paper-denominator Leaven result JSONL rows exist.",
        "acceptance": acceptance,
        "dataset_manifest_summary": {
            "case_count": dataset_manifest["case_count"],
            "dataset_json_sha256": dataset_manifest["dataset_json"]["sha256"],
            "case_order_sha256": dataset_manifest["case_order"]["sha256"],
            "splits": dataset_manifest["splits"],
            "referenced_workbook_missing_directory_count": dataset_manifest["referenced_workbooks"][
                "missing_directory_count"
            ],
        },
        "result_jsonl_files": [rel(path, repo_root) for path in result_jsonl],
        "result_record_summary": {
            "total_records": len(result_records),
            "non_overlay_records": sum(1 for record in result_records if record.get("plot_binding") is None),
            "paper_denominator_records": len(paper_denominator_result_records),
        },
        "reproduced_denominators": reproduced_denominators,
        "missing_denominators": missing_denominators,
        "forbidden_proxy_completion_labels": FORBIDDEN_PROXY_LABELS,
    }


def write_markdown(report: dict[str, Any], output: Path) -> None:
    lines = [
        "# Trace2Skill Closeout Audit",
        "",
        f"Overall complete: `{str(report['overall_complete']).lower()}`",
        "",
        report["reason"],
        "",
        "## Acceptance Status",
        "",
        "| Acceptance id | Status | Remaining |",
        "|---------------|--------|-----------|",
    ]
    for acceptance_id in ACCEPTANCE_IDS:
        item = report["acceptance"][acceptance_id]
        remaining = "<br>".join(item["remaining"]) if item["remaining"] else ""
        lines.append(f"| `{acceptance_id}` | `{item['status']}` | {remaining} |")

    lines.extend(
        [
            "",
            "## Current Denominators",
            "",
            "Reproduced or captured:",
        ]
    )
    lines.extend(f"- `{item}`" for item in report["reproduced_denominators"])
    lines.extend(["", "Missing:"])
    lines.extend(f"- `{item}`" for item in report["missing_denominators"])
    lines.extend(
        [
            "",
            "## Result Records",
            "",
            f"- JSONL files: `{len(report['result_jsonl_files'])}`",
            f"- Total rows: `{report['result_record_summary']['total_records']}`",
            f"- Non-overlay rows: `{report['result_record_summary']['non_overlay_records']}`",
            f"- Paper-denominator rows: `{report['result_record_summary']['paper_denominator_records']}`",
            "",
            "## Dataset Manifest",
            "",
            f"- Case count: `{report['dataset_manifest_summary']['case_count']}`",
            f"- Dataset JSON SHA-256: `{report['dataset_manifest_summary']['dataset_json_sha256']}`",
            f"- Case order SHA-256: `{report['dataset_manifest_summary']['case_order_sha256']}`",
            f"- Missing workbook directories: `{report['dataset_manifest_summary']['referenced_workbook_missing_directory_count']}`",
            "",
            "## Proxy Refusal",
            "",
            "These labels remain forbidden as full reproduction closeout evidence:",
        ]
    )
    lines.extend(f"- `{item}`" for item in report["forbidden_proxy_completion_labels"])
    output.write_text("\n".join(lines) + "\n", encoding="utf-8")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "ara_dir",
        type=Path,
        default=Path("docs/ara/trace2skill_spreadsheetbench"),
        nargs="?",
    )
    parser.add_argument(
        "--output-json",
        type=Path,
        default=Path("docs/ara/trace2skill_spreadsheetbench/results/closeout_audit.json"),
    )
    parser.add_argument(
        "--output-md",
        type=Path,
        default=Path("docs/ara/trace2skill_spreadsheetbench/results/closeout_audit.md"),
    )
    args = parser.parse_args()

    repo_root = Path.cwd().resolve()
    ara_dir = args.ara_dir.resolve()
    report = audit(repo_root, ara_dir)
    args.output_json.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    write_markdown(report, args.output_md)
    print(args.output_json)
    print(args.output_md)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
