#!/usr/bin/env python3
"""Build a denominator-labeled JSONL result for the deterministic Trace2Skill one-case proof."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any


def load_json(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def require_file(path: Path, label: str) -> None:
    if not path.is_file():
        raise FileNotFoundError(f"missing {label}: {path}")


def build_record(run_dir: Path, created_at: str | None) -> dict[str, Any]:
    score_path = run_dir / "score_report.json"
    manifest_path = run_dir / "manifest.json"
    trajectory_path = run_dir / "trajectory.json"
    acp_path = run_dir / "acp_result.json"
    transcript_path = run_dir / "agent_transcript.md"
    worker_path = run_dir / "trace2skill_acp_worker.py"
    prompt_path = run_dir / "agent_prompt.md"
    for label, path in (
        ("prepared prompt", prompt_path),
        ("score report", score_path),
        ("manifest", manifest_path),
        ("trajectory", trajectory_path),
        ("ACP result", acp_path),
        ("transcript", transcript_path),
        ("worker script", worker_path),
    ):
        require_file(path, label)

    score = load_json(score_path)
    manifest = load_json(manifest_path)
    trajectory = load_json(trajectory_path)
    acp = load_json(acp_path)

    output_workbook = Path(score["candidate_workbook"]["path"])
    init_workbook = Path(manifest["init_workbook"])
    golden_workbook = Path(score["golden_workbook"]["path"])
    require_file(init_workbook, "init workbook")
    require_file(golden_workbook, "golden workbook")
    require_file(output_workbook, "output workbook")
    if score.get("passed") is not True:
        raise ValueError("score_report.json does not mark the one-case result as passed")
    if score.get("case_id") != "13-1":
        raise ValueError(f"expected case_id 13-1, got {score.get('case_id')!r}")

    cost = acp.get("primary", {}).get("cost", {})
    model_id = trajectory.get("model_id") or "local-openpyxl-trace2skill-agent"
    completed_at = acp.get("receipts", [{}])[0].get("completed_at")
    result_created_at = created_at or completed_at
    if not result_created_at:
        raise ValueError("created_at was not provided and ACP receipt has no completed_at timestamp")

    return {
        "schema_version": "leaven.trace2skill.result.v1",
        "run_id": "trace2skill-one-case-13-1-local-openpyxl",
        "created_at": result_created_at,
        "proof_classification": "deterministic-one-case",
        "dataset_slice": {
            "name": "SpreadsheetBench-Verified",
            "split": "one-case",
            "case_range": "0..1",
            "case_count": 1,
            "denominator": "one-case-13-1-only",
        },
        "model_id": model_id,
        "serving_backend": "local-deterministic-acp-worker",
        "seed": None,
        "skill_source": {
            "kind": "deterministic-openpyxl-worker",
            "path": worker_path.as_posix(),
        },
        "metric_name": "workbook_score",
        "metric_value": float(score["score"]),
        "metric_unit": "fraction",
        "plot_binding": None,
        "cost": {
            "usd": 0,
            "agent_calls": cost.get("agent_calls"),
            "usd_micro": cost.get("usd_micro"),
        },
        "runtime": {
            "seconds": None,
            "workers": 1,
        },
        "source_command": "cargo run -p trace2skill_spreadsheetbench -- --run-one-case-acp-worker --run-dir tmp/trace2skill-one-case-live --model-id local-openpyxl-trace2skill-agent",
        "artifact_paths": [
            prompt_path.as_posix(),
            init_workbook.as_posix(),
            golden_workbook.as_posix(),
            manifest_path.as_posix(),
            acp_path.as_posix(),
            transcript_path.as_posix(),
            score_path.as_posix(),
            trajectory_path.as_posix(),
            output_workbook.as_posix(),
            worker_path.as_posix(),
        ],
        "notes": "Deterministic one-case result record. plot_binding is null by design: this row must not be overlaid on paper-target SpreadsheetBench/Qwen/vLLM plots.",
        "extra": {
            "runbook_stage_id": "G1",
            "case_id": score["case_id"],
            "answer_sheet": score["answer_sheet"],
            "answer_position": score["answer_position"],
            "matched_cells": score["matched_cells"],
            "total_cells": score["total_cells"],
            "manifest_status": manifest.get("status"),
        },
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--run-dir", type=Path, default=Path("tmp/trace2skill-one-case-live"))
    parser.add_argument(
        "--output",
        type=Path,
        default=Path("docs/ara/trace2skill_spreadsheetbench/results/deterministic_one_case.jsonl"),
    )
    parser.add_argument("--created-at")
    args = parser.parse_args()

    record = build_record(args.run_dir, args.created_at)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(record, sort_keys=True) + "\n", encoding="utf-8")
    print(args.output)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
