#!/usr/bin/env python3
"""Run the canonical test suite and enforce its wall-clock runtime SLA."""

from __future__ import annotations

import argparse
import subprocess
import time


TEST_COMMANDS = (
    ("nextest workspace suite", ["cargo", "nextest", "run", "--workspace"]),
    ("workspace doctests", ["cargo", "test", "--workspace", "--doc"]),
)


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Run the full test suite and fail if it exceeds the runtime SLA."
    )
    parser.add_argument(
        "--sla-seconds",
        type=float,
        default=30.0,
        help="maximum allowed wall-clock runtime for the full test suite",
    )
    args = parser.parse_args()

    started = time.perf_counter()
    for label, command in TEST_COMMANDS:
        print(f"running {label}: {' '.join(command)}", flush=True)
        result = subprocess.run(command, check=False)
        if result.returncode != 0:
            return result.returncode

    elapsed = time.perf_counter() - started
    print(f"test suite runtime: {elapsed:.2f}s (SLA < {args.sla_seconds:.2f}s)")
    if elapsed >= args.sla_seconds:
        print(
            f"error: full test suite exceeded runtime SLA "
            f"({elapsed:.2f}s >= {args.sla_seconds:.2f}s)"
        )
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
