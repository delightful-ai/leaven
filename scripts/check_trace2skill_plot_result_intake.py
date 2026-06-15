#!/usr/bin/env python3
"""Check that Trace2Skill plot overlays must pass result intake."""

from __future__ import annotations

import argparse
import json
import subprocess
from pathlib import Path


def invalid_overlay_row() -> dict[str, object]:
    return {
        "schema_version": "leaven.trace2skill.result.v1",
        "run_id": "trace2skill-invalid-overlay-fixture",
        "created_at": "2026-06-14T00:00:05Z",
        "proof_classification": "paper-subset",
        "dataset_slice": {
            "name": "SpreadsheetBench-Verified",
            "split": "held_out",
            "case_range": "200..202",
            "case_count": 2,
            "denominator": "fixture-held-out-subset-not-paper",
        },
        "model_id": "fixture-model",
        "serving_backend": "fixture-backend",
        "seed": 41,
        "skill_source": {"kind": "fixture-skill"},
        "metric_name": "official_instance_accuracy",
        "metric_value": 50.0,
        "metric_unit": "percent",
        "plot_binding": {
            "panel": "same_model_deepening_vrf",
            "x_label": "+Combined\n122B",
            "series": "Invalid fixture",
            "axis": "left",
        },
        "cost": {
            "usd": None,
            "prompt_tokens": None,
            "completion_tokens": None,
        },
        "runtime": {
            "seconds": None,
            "workers": 128,
            "max_turns": 100,
        },
        "source_command": "python run_spreadsheetbench.py && python evaluate_with_official.py",
        "artifact_paths": [
            "docs/ara/trace2skill_spreadsheetbench/results/full_run_plan.md",
        ],
        "extra": {},
        "notes": "Invalid overlay fixture: shape-valid for plotting but missing runbook admission.",
    }


def check_plot_result_intake(repo_root: Path, ara_root: Path) -> list[str]:
    errors: list[str] = []
    target_dir = repo_root / "target/trace2skill-plot-result-intake"
    target_dir.mkdir(parents=True, exist_ok=True)
    invalid_results = target_dir / "invalid-overlay.jsonl"
    invalid_results.write_text(json.dumps(invalid_overlay_row(), sort_keys=True) + "\n", encoding="utf-8")
    output = target_dir / "invalid-overlay.png"

    command = [
        "uv",
        "run",
        "--with",
        "matplotlib",
        "--with",
        "pandas",
        "python",
        str(repo_root / "scripts/plot_trace2skill_ara.py"),
        str(ara_root),
        "--results",
        str(invalid_results),
        "--output",
        str(output),
    ]
    result = subprocess.run(command, cwd=repo_root, text=True, capture_output=True, check=False)
    expected = "result overlays failed result intake"
    if result.returncode == 0:
        errors.append("plotter unexpectedly accepted an overlay row that fails result intake")
    elif expected not in result.stderr and expected not in result.stdout:
        detail = "\n".join(part for part in [result.stdout.strip(), result.stderr.strip()] if part)
        errors.append(f"plotter failed for the wrong reason; expected {expected!r}, got {detail!r}")
    if output.exists():
        errors.append(f"plotter wrote an output image for an invalid overlay: {output}")
    return errors


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("ara_dir", type=Path)
    args = parser.parse_args()

    ara_root = args.ara_dir.resolve()
    repo_root = Path(__file__).resolve().parents[1]
    errors = check_plot_result_intake(repo_root, ara_root)
    if errors:
        for error in errors:
            print(f"ERROR: {error}")
        return 1
    print(f"PASS: {args.ara_dir} plot result-intake gate")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
