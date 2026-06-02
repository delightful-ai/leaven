#!/usr/bin/env python3
"""Run the canonical test suite and report its wall-clock runtime target."""

from __future__ import annotations

import argparse
import os
from pathlib import Path
import re
import signal
import shutil
import subprocess
import time

from workspace_packages import MILESTONE_PACKAGES, cargo_metadata, package_exclude_args


NEXTEST_WORKSPACE_COMMAND = [
    "cargo",
    "nextest",
    "run",
    "--workspace",
    "--all-targets",
    *package_exclude_args(MILESTONE_PACKAGES),
]
NEXTEST_BUILD_COMMAND = [*NEXTEST_WORKSPACE_COMMAND, "--no-run"]
NEXTEST_RUN_COMMAND = [
    *NEXTEST_WORKSPACE_COMMAND,
    "--failure-output",
    "immediate-final",
    "--status-level",
    "slow",
    "--final-status-level",
    "slow",
]

DEFAULT_BUILD_DISCOVERY_TIMEOUT_SECONDS = 300.0

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
    payload = cargo_metadata(workspace_root)
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
    return workspace_doctest_commands(workspace_root)


def run_process_group_with_timeout(
    label: str,
    command: list[str],
    cwd: Path,
    timeout: float | None,
    stdout: int | None = None,
) -> int:
    process = subprocess.Popen(command, cwd=cwd, start_new_session=True, stdout=stdout)
    started = time.perf_counter()
    while True:
        returncode = process.poll()
        if returncode is not None:
            return returncode
        if timeout is not None and time.perf_counter() - started >= timeout:
            timeout_label = f"{timeout:.2f}s"
            print(f"error: {label} exceeded timeout ({timeout_label})", flush=True)
            try:
                os.killpg(process.pid, signal.SIGTERM)
            except ProcessLookupError:
                return process.wait()
            try:
                process.wait(timeout=2.0)
                return 1
            except subprocess.TimeoutExpired:
                try:
                    os.killpg(process.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                process.wait()
                return 1
        time.sleep(0.05)


def ensure_nextest_available() -> None:
    if shutil.which("cargo-nextest") is None:
        raise SystemExit(
            "error: cargo-nextest is required for `just test`; install it with "
            "`cargo install cargo-nextest --locked`"
        )


def build_workspace_tests(workspace_root: Path, build_timeout: float | None = None) -> int:
    print(
        "building workspace libtest suite with nextest: "
        + " ".join(NEXTEST_BUILD_COMMAND),
        flush=True,
    )
    return run_process_group_with_timeout(
        "workspace nextest build",
        NEXTEST_BUILD_COMMAND,
        workspace_root,
        build_timeout,
    )


def run_with_deadline(label: str, command: list[str], cwd: Path, deadline: float) -> int:
    remaining = deadline - time.perf_counter()
    if remaining <= 0:
        print(f"error: no SLA time remains before starting {label}", flush=True)
        return 1

    return run_process_group_with_timeout(label, command, cwd, remaining)


def prewarm_commands(commands: list[tuple[str, list[str]]], workspace_root: Path) -> int:
    for label, command in commands:
        print(f"prewarming {label}: {' '.join(command)}", flush=True)
        returncode = subprocess.run(command, cwd=workspace_root, check=False).returncode
        if returncode != 0:
            return returncode
    return 0


def run_workspace_libtests(deadline: float, workspace_root: Path) -> int:
    print(
        "running workspace libtests with nextest: " + " ".join(NEXTEST_RUN_COMMAND),
        flush=True,
    )
    return run_with_deadline(
        "workspace nextest run",
        NEXTEST_RUN_COMMAND,
        workspace_root,
        deadline,
    )


def main() -> int:
    parser = argparse.ArgumentParser(
        description="Run the full test suite and report whether it exceeds the runtime target."
    )
    parser.add_argument(
        "--warn-seconds",
        type=float,
        default=30.0,
        help="wall-clock runtime target for executing the full test suite",
    )
    parser.add_argument(
        "--timeout-seconds",
        type=float,
        default=600.0,
        help="hard timeout for executing the full test suite",
    )
    parser.add_argument(
        "--build-timeout",
        type=float,
        default=DEFAULT_BUILD_DISCOVERY_TIMEOUT_SECONDS,
        help=(
            "optional hang guard for compiling workspace libtests; "
            "this is separate from the runtime target"
        ),
    )
    args = parser.parse_args()

    workspace_root = Path.cwd()
    ensure_nextest_available()
    returncode = build_workspace_tests(workspace_root, args.build_timeout)
    if returncode != 0:
        return returncode
    commands = test_commands(workspace_root)
    returncode = prewarm_commands(commands, workspace_root)
    if returncode != 0:
        return returncode

    started = time.perf_counter()
    deadline = started + args.timeout_seconds

    for label, command in commands:
        print(f"running {label}: {' '.join(command)}", flush=True)
        returncode = run_with_deadline(label, command, workspace_root, deadline)
        if returncode != 0:
            return returncode

    returncode = run_workspace_libtests(deadline, workspace_root)
    if returncode != 0:
        return returncode

    elapsed = time.perf_counter() - started
    print(
        f"test suite runtime: {elapsed:.2f}s "
        f"(target < {args.warn_seconds:.2f}s, timeout < {args.timeout_seconds:.2f}s)"
    )
    if elapsed >= args.warn_seconds:
        print(
            f"warning: full test suite exceeded runtime target "
            f"({elapsed:.2f}s >= {args.warn_seconds:.2f}s)"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
