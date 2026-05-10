#!/usr/bin/env python3
"""Run cargo-llvm-cov and enforce line plus branch coverage floors."""

from __future__ import annotations

import argparse
import json
import shutil
import subprocess
from pathlib import Path
from typing import Any

RUN_PACKAGES = [
    "p0_graph_skeleton",
    "p1_keep_best",
    "p2_pairwise_tournament",
    "p3_gepa_parity",
    "p4_meta_harness_lite",
    "p5_evoskill_iteration",
    "p6_optimizer_policy_self_opt",
    "p7_self_optimization_kernel",
    "p8_aime_gepa",
    "xtask",
]

LIVE_PACKAGES = [
    "p5_evoskill_iteration",
]


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Run workspace coverage and fail below configured floors."
    )
    parser.add_argument("--line-floor", type=float, required=True)
    parser.add_argument("--branch-floor", type=float, required=True)
    parser.add_argument(
        "--output-path",
        default="target/llvm-cov/coverage-summary.json",
        help="where to write the llvm-cov JSON summary",
    )
    args = parser.parse_args()
    output_path = Path(args.output_path)
    output_path.parent.mkdir(parents=True, exist_ok=True)
    p5_run_dir = Path("target/llvm-cov-p5-evoskill")
    shutil.rmtree(p5_run_dir, ignore_errors=True)

    commands = [
        ["cargo", "llvm-cov", "clean", "--workspace"],
        [
            "cargo",
            "llvm-cov",
            "--workspace",
            "--no-report",
            "--branch",
            *exclude_args(),
        ],
        *[run_package_command(package, p5_run_dir) for package in RUN_PACKAGES],
    ]
    for command in commands:
        result = run(command)
        if result.returncode != 0:
            return result.returncode

    lcov_path = output_path.with_suffix(".lcov")
    lcov_command = [
        "cargo",
        "llvm-cov",
        "report",
        "--lcov",
        "--branch",
        "--output-path",
        str(lcov_path),
    ]
    result = run(lcov_command)
    if result.returncode != 0:
        return result.returncode

    report_command = [
        "cargo",
        "llvm-cov",
        "report",
        "--json",
        "--summary-only",
        "--branch",
        "--output-path",
        str(output_path),
    ]
    result = run(report_command)
    if result.returncode != 0:
        return result.returncode

    summary = load_summary(output_path)
    lines = load_lcov_lines(lcov_path)
    branches = summary["branches"]

    print(
        "line coverage: "
        f"{lines['percent']:.2f}% "
        f"({lines['covered']}/{lines['count']}, floor {args.line_floor:.2f}%)"
    )
    print(
        "branch coverage: "
        f"{branches['percent']:.2f}% "
        f"({branches['covered']}/{branches['count']}, floor {args.branch_floor:.2f}%)"
    )

    if lines["count"] == 0:
        print("error: line coverage produced no line denominator")
        return 1
    if lines["percent"] < args.line_floor:
        print(
            "error: line coverage below floor "
            f"({lines['percent']:.2f}% < {args.line_floor:.2f}%)"
        )
        return 1
    if branches["count"] == 0:
        print("error: branch coverage produced no branch denominator")
        return 1
    if branches["percent"] < args.branch_floor:
        print(
            "error: branch coverage below floor "
            f"({branches['percent']:.2f}% < {args.branch_floor:.2f}%)"
        )
        return 1
    return 0


def run(command: list[str]) -> subprocess.CompletedProcess[bytes]:
    print(f"running coverage gate: {' '.join(command)}", flush=True)
    return subprocess.run(command, check=False)


def run_package_command(package: str, p5_run_dir: Path) -> list[str]:
    command = ["cargo", "llvm-cov", "run", "--no-report", "-p", package]
    if package == "p5_evoskill_iteration":
        command.extend(["--", "--run-dir", str(p5_run_dir)])
    return command


def exclude_args() -> list[str]:
    args: list[str] = []
    for package in LIVE_PACKAGES:
        args.extend(["--exclude", package])
    return args


def load_summary(path: Path) -> dict[str, Any]:
    payload = json.loads(path.read_text(encoding="utf-8"))
    data = payload.get("data")
    if not isinstance(data, list) or not data:
        raise ValueError(f"coverage summary at {path} has no data")
    totals = data[0].get("totals")
    if not isinstance(totals, dict):
        raise ValueError(f"coverage summary at {path} has no totals")
    return totals


def load_lcov_lines(path: Path) -> dict[str, Any]:
    count = 0
    covered = 0
    for line in path.read_text(encoding="utf-8").splitlines():
        if not line.startswith("DA:"):
            continue
        count += 1
        _, hits, *_ = line[3:].split(",")
        if int(hits) > 0:
            covered += 1
    percent = 100.0 * covered / count if count else 0.0
    return {"count": count, "covered": covered, "percent": percent}


if __name__ == "__main__":
    raise SystemExit(main())
