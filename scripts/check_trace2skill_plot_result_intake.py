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


def touch(repo_root: Path, path: Path, text: str) -> str:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")
    return path.relative_to(repo_root).as_posix()


def blocked_approval_overlay_row(repo_root: Path, target_dir: Path) -> dict[str, object]:
    prompt = touch(
        repo_root,
        target_dir / "blocked_overlay/rendered_prompts/52807/agent_prompt.md",
        "fixture rendered prompt\n",
    )
    prompt_manifest = touch(
        repo_root,
        target_dir / "blocked_overlay/prompt_render_manifest.json",
        '{"schema_version":"fixture.prompt_manifest.v1"}\n',
    )
    eval_results = touch(
        repo_root,
        target_dir / "blocked_overlay/outputs/eval_official_results.json",
        '{"schema_version":"fixture.eval.v1"}\n',
    )
    approval_artifact = "docs/ara/trace2skill_spreadsheetbench/results/full_run_plan.md"
    return {
        "schema_version": "leaven.trace2skill.result.v1",
        "run_id": "trace2skill-blocked-approval-overlay-fixture",
        "created_at": "2026-06-14T00:00:06Z",
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
            "series": "Blocked approval fixture",
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
        "source_command": "python run_spreadsheetbench.py --start_idx 200 --end_idx 202 && python evaluate_with_official.py --start_idx 200 --end_idx 202",
        "artifact_paths": [approval_artifact, prompt, prompt_manifest, eval_results],
        "extra": {
            "runbook_stage_id": "G2",
            "approval_artifact_paths": [approval_artifact],
            "command_policy": "upstream-eval",
        },
        "notes": "Shape-valid overlay fixture that must not plot while approval is blocked.",
    }


def run_plotter(repo_root: Path, ara_root: Path, results: Path, output: Path) -> subprocess.CompletedProcess[str]:
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
        str(results),
        "--output",
        str(output),
    ]
    return subprocess.run(command, cwd=repo_root, text=True, capture_output=True, check=False)


def expect_plotter_refusal(
    repo_root: Path,
    ara_root: Path,
    results: Path,
    output: Path,
    expected: str,
    errors: list[str],
    label: str,
) -> None:
    result = run_plotter(repo_root, ara_root, results, output)
    if result.returncode == 0:
        errors.append(f"{label}: plotter unexpectedly accepted invalid overlay")
    elif expected not in result.stderr and expected not in result.stdout:
        detail = "\n".join(part for part in [result.stdout.strip(), result.stderr.strip()] if part)
        errors.append(f"{label}: expected {expected!r}, got {detail!r}")
    if output.exists():
        errors.append(f"{label}: plotter wrote an output image for an invalid overlay: {output}")


def check_plot_result_intake(repo_root: Path, ara_root: Path) -> list[str]:
    errors: list[str] = []
    target_dir = repo_root / "target/trace2skill-plot-result-intake"
    target_dir.mkdir(parents=True, exist_ok=True)
    invalid_results = target_dir / "invalid-overlay.jsonl"
    invalid_results.write_text(json.dumps(invalid_overlay_row(), sort_keys=True) + "\n", encoding="utf-8")
    output = target_dir / "invalid-overlay.png"

    expect_plotter_refusal(
        repo_root,
        ara_root,
        invalid_results,
        output,
        "result overlays failed result intake",
        errors,
        "invalid runbook overlay",
    )

    blocked_results = ara_root / "results/fixture_blocked_approval_overlay.jsonl"
    blocked_output = target_dir / "blocked-approval-overlay.png"
    try:
        blocked_results.write_text(
            json.dumps(blocked_approval_overlay_row(repo_root, target_dir), sort_keys=True) + "\n",
            encoding="utf-8",
        )
        expect_plotter_refusal(
            repo_root,
            ara_root,
            blocked_results,
            blocked_output,
            "require a runnable approval packet",
            errors,
            "blocked approval overlay",
        )
    finally:
        blocked_results.unlink(missing_ok=True)
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
