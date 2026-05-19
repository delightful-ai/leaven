#!/usr/bin/env python3
"""Run the canonical test suite and enforce its wall-clock runtime SLA."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import re
import subprocess
import time


MILESTONE_PACKAGES = [
    "p0_graph_skeleton",
    "p1_keep_best",
    "p2_pairwise_tournament",
    "p3_gepa_parity",
    "p4_meta_harness_lite",
    "p5_evoskill_iteration",
    "p6_optimizer_policy_self_opt",
    "p7_self_optimization_kernel",
    "p8_aime_gepa",
]

NEXTTEST_COMMAND = (
    "nextest workspace suite",
    [
        "cargo",
        "nextest",
        "run",
        "--workspace",
        *[arg for package in MILESTONE_PACKAGES for arg in ("--exclude", package)],
    ],
)

NON_RUST_FENCE_LANGUAGES = {
    "console",
    "json",
    "md",
    "markdown",
    "sh",
    "shell",
    "text",
    "toml",
    "txt",
    "yaml",
    "yml",
}


def package_has_rust_doctest(package_root: Path) -> bool:
    """Return whether package Rust docs contain a Rust doctest code fence."""
    src = package_root / "src"
    if not src.exists():
        return False
    for path in src.rglob("*.rs"):
        if rust_source_has_doctest(path):
            return True
    return False


def rust_source_has_doctest(path: Path) -> bool:
    fence = re.compile(r"^\s*(?:///|//!)\s*```(?P<info>.*)$")
    in_non_rust_fence = False
    for line in path.read_text(encoding="utf-8").splitlines():
        match = fence.match(line)
        if match is None:
            continue
        info = match.group("info").strip().lower()
        if in_non_rust_fence:
            if not info:
                in_non_rust_fence = False
            continue
        if not info:
            return True
        first = re.split(r"[\s,]+", info, maxsplit=1)[0]
        if first == "rust" or first in {"ignore", "no_run", "should_panic", "compile_fail"}:
            return True
        if first in NON_RUST_FENCE_LANGUAGES:
            in_non_rust_fence = True
            continue
        if not first.startswith("edition"):
            return True
    return False


def workspace_doctest_commands(workspace_root: Path) -> list[tuple[str, list[str]]]:
    metadata = subprocess.run(
        ["cargo", "metadata", "--no-deps", "--format-version", "1"],
        cwd=workspace_root,
        check=False,
        stdout=subprocess.PIPE,
        text=True,
    )
    if metadata.returncode != 0:
        raise SystemExit(metadata.returncode)
    payload = json.loads(metadata.stdout)
    members = set(payload["workspace_members"])
    names: list[str] = []
    for package in payload["packages"]:
        if package["name"] in MILESTONE_PACKAGES:
            continue
        if package["id"] not in members:
            continue
        has_library_target = any("lib" in target["kind"] for target in package["targets"])
        if not has_library_target:
            continue
        package_root = Path(package["manifest_path"]).parent
        if package_has_rust_doctest(package_root):
            names.append(package["name"])
    if not names:
        return []
    command = ["cargo", "test", "--doc"]
    for name in names:
        command.extend(("-p", name))
    return [("workspace doctests", command)]


def test_commands(workspace_root: Path) -> list[tuple[str, list[str]]]:
    commands = [NEXTTEST_COMMAND]
    doctests = workspace_doctest_commands(workspace_root)
    if doctests:
        commands.extend(doctests)
    else:
        print("skipping workspace doctests: no Rust doctest fences found", flush=True)
    return commands


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
    workspace_root = Path.cwd()
    for label, command in test_commands(workspace_root):
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
