#!/usr/bin/env python3
"""Run cargo-llvm-cov and enforce line plus branch coverage floors."""

from __future__ import annotations

import argparse
import json
import subprocess
from pathlib import Path
from typing import Any


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Run workspace coverage and fail below configured floors."
    )
    parser.add_argument("--line-floor", type=float, required=True)
    parser.add_argument("--branch-floor", type=float, required=True)
    parser.add_argument("--ignore-filename-regex", required=True)
    parser.add_argument(
        "--output-path",
        default="target/llvm-cov/coverage-summary.json",
        help="where to write the llvm-cov JSON summary",
    )
    args = parser.parse_args()
    output_path = Path(args.output_path)
    output_path.parent.mkdir(parents=True, exist_ok=True)

    command = [
        "cargo",
        "llvm-cov",
        "--workspace",
        "--json",
        "--summary-only",
        "--branch",
        "--ignore-filename-regex",
        args.ignore_filename_regex,
        "--fail-under-lines",
        str(args.line_floor),
        "--output-path",
        str(output_path),
    ]
    print(f"running coverage gate: {' '.join(command)}", flush=True)
    result = subprocess.run(command, check=False)
    if result.returncode != 0:
        return result.returncode

    summary = load_summary(output_path)
    lines = summary["lines"]
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


def load_summary(path: Path) -> dict[str, Any]:
    payload = json.loads(path.read_text(encoding="utf-8"))
    data = payload.get("data")
    if not isinstance(data, list) or not data:
        raise ValueError(f"coverage summary at {path} has no data")
    totals = data[0].get("totals")
    if not isinstance(totals, dict):
        raise ValueError(f"coverage summary at {path} has no totals")
    return totals


if __name__ == "__main__":
    raise SystemExit(main())
