#!/usr/bin/env python3
"""Run cargo-llvm-cov and enforce line plus branch coverage floors."""

from __future__ import annotations

import argparse
import re
import subprocess
from pathlib import Path
from typing import Any

# The dev/test profiles select the Cranelift codegen backend for workspace
# crates (see root Cargo.toml). Coverage instrumentation (`-Cinstrument-coverage`)
# requires the LLVM backend, so every build/report command runs under the
# `coverage` profile, which inherits `dev` but pins `codegen-backend = llvm`.
COVERAGE_PROFILE = ["--profile", "coverage"]

MILESTONE_PACKAGES = [
    "p0_graph_skeleton",
    "p1_keep_best",
    "p2_pairwise_tournament",
    "p3_gepa_parity",
    "p4_meta_harness_lite",
    "p6_optimizer_policy_self_opt",
    "p7_self_optimization_kernel",
    "p8_aime_gepa",
]

RUN_PACKAGES = [
    "xtask",
]

TEST_SOURCE_RE = re.compile(r"(^|/)(tests|benches)/|(^|/)target/tests/")


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
    commands = [
        ["cargo", "llvm-cov", "clean", "--workspace"],
        [
            "cargo",
            "llvm-cov",
            "--workspace",
            "--no-report",
            "--branch",
            *COVERAGE_PROFILE,
            *exclude_args(),
        ],
        *[run_package_command(package) for package in RUN_PACKAGES],
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
        *COVERAGE_PROFILE,
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
        *COVERAGE_PROFILE,
        "--output-path",
        str(output_path),
    ]
    result = run(report_command)
    if result.returncode != 0:
        return result.returncode

    coverage = load_lcov_coverage(lcov_path)
    lines = coverage["lines"]
    branches = coverage["branches"]

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


def run_package_command(package: str) -> list[str]:
    return ["cargo", "llvm-cov", "run", "--no-report", *COVERAGE_PROFILE, "-p", package]


def exclude_args() -> list[str]:
    args: list[str] = []
    for package in MILESTONE_PACKAGES:
        args.extend(["--exclude", package])
    return args


def load_lcov_coverage(path: Path) -> dict[str, Any]:
    line_count = 0
    line_covered = 0
    branch_count = 0
    branch_covered = 0
    current_source: Path | None = None
    test_ranges: list[tuple[int, int]] = []
    for line in path.read_text(encoding="utf-8").splitlines():
        if line.startswith("SF:"):
            current_source = Path(line[3:])
            test_ranges = source_test_ranges(current_source)
            continue
        if current_source is None or TEST_SOURCE_RE.search(current_source.as_posix()):
            continue
        if line.startswith("DA:"):
            raw_line, hits, *_ = line[3:].split(",")
            source_line = int(raw_line)
            if in_ranges(source_line, test_ranges):
                continue
            line_count += 1
            if int(hits) > 0:
                line_covered += 1
            continue
        if line.startswith("BRDA:"):
            raw_line, _, _, hits = line[5:].split(",")
            if hits == "-":
                continue
            source_line = int(raw_line)
            if in_ranges(source_line, test_ranges):
                continue
            branch_count += 1
            if int(hits) > 0:
                branch_covered += 1
    line_percent = 100.0 * line_covered / line_count if line_count else 0.0
    branch_percent = 100.0 * branch_covered / branch_count if branch_count else 0.0
    return {
        "lines": {
            "count": line_count,
            "covered": line_covered,
            "percent": line_percent,
        },
        "branches": {
            "count": branch_count,
            "covered": branch_covered,
            "percent": branch_percent,
        },
    }


def source_test_ranges(path: Path) -> list[tuple[int, int]]:
    if not path.is_file():
        return []
    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except UnicodeDecodeError:
        return []

    ranges: list[tuple[int, int]] = []
    pending_cfg_test = False
    for index, line in enumerate(lines, start=1):
        stripped = line.strip()
        if stripped.startswith("#[cfg(test)]"):
            pending_cfg_test = True
            continue
        if not pending_cfg_test:
            continue
        if not stripped or stripped.startswith("#["):
            continue
        if re.match(r"(?:pub(?:\([^)]*\))?\s+)?mod\s+\w+\s*\{", stripped):
            ranges.append(module_range(lines, index))
        pending_cfg_test = False
    return ranges


def module_range(lines: list[str], start: int) -> tuple[int, int]:
    depth = 0
    seen_open = False
    for index in range(start, len(lines) + 1):
        line = lines[index - 1]
        depth += line.count("{")
        if "{" in line:
            seen_open = True
        depth -= line.count("}")
        if seen_open and depth <= 0:
            return (start, index)
    return (start, len(lines))


def in_ranges(line: int, ranges: list[tuple[int, int]]) -> bool:
    return any(start <= line <= end for start, end in ranges)


if __name__ == "__main__":
    raise SystemExit(main())
