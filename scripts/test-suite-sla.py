#!/usr/bin/env python3
"""Run the canonical test suite and enforce its wall-clock runtime SLA."""

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import re
import signal
import subprocess
import sys
import tempfile
import time


MILESTONE_PACKAGES = [
    "p0_graph_skeleton",
    "p1_keep_best",
    "p2_pairwise_tournament",
    "p3_gepa_parity",
    "p4_meta_harness_lite",
    "p5_evoskill_iteration",
    "p5_skill_paper_reproductions",
    "p6_optimizer_policy_self_opt",
    "p7_self_optimization_kernel",
    "p8_aime_gepa",
    "trace2skill_spreadsheetbench",
]

WORKSPACE_TEST_BUILD_COMMAND = [
    "cargo",
    "test",
    "--no-run",
    "--message-format=json",
    "--workspace",
    *[arg for package in MILESTONE_PACKAGES for arg in ("--exclude", package)],
    "--lib",
    "--tests",
]

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

TEST_MARKER_RE = re.compile(
    r"#\s*\[\s*(?:tokio::)?test\b|proptest!|rstest\b|test_case\b|quickcheck\b"
)


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


def rust_source_has_test_marker(path: Path) -> bool:
    try:
        return bool(TEST_MARKER_RE.search(path.read_text(encoding="utf-8")))
    except UnicodeDecodeError:
        return False


def rust_tree_has_test_marker(path: Path) -> bool:
    if path.is_file():
        if rust_source_has_test_marker(path):
            return True
        module_dir = path.with_suffix("")
        if module_dir.is_dir():
            return any(
                rust_source_has_test_marker(module_path)
                for module_path in module_dir.rglob("*.rs")
            )
        return False
    if path.is_dir():
        return any(rust_source_has_test_marker(module_path) for module_path in path.rglob("*.rs"))
    return False


def package_source_has_test_marker(package_root: Path) -> bool:
    src = package_root / "src"
    if not src.exists():
        return False
    return rust_tree_has_test_marker(src)


def target_has_tests(message: dict[str, object], package_root: Path) -> bool:
    target = message["target"]
    assert isinstance(target, dict)
    kinds = set(target.get("kind", []))
    src_path = target.get("src_path")
    if "test" in kinds:
        return isinstance(src_path, str) and rust_tree_has_test_marker(Path(src_path))
    if kinds.intersection({"lib", "bin", "proc-macro"}):
        return package_source_has_test_marker(package_root)
    return True


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
    return workspace_doctest_commands(workspace_root)


def workspace_package_roots(workspace_root: Path) -> dict[str, Path]:
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
    return {
        package["id"]: Path(package["manifest_path"]).parent
        for package in payload["packages"]
    }


def rust_target_libdir(workspace_root: Path) -> Path:
    result = subprocess.run(
        ["rustc", "--print", "target-libdir"],
        cwd=workspace_root,
        check=False,
        stdout=subprocess.PIPE,
        text=True,
    )
    if result.returncode != 0:
        raise SystemExit(result.returncode)
    return Path(result.stdout.strip())


def discover_workspace_test_binaries(
    workspace_root: Path, deadline: float
) -> list[tuple[str, Path, Path]]:
    remaining = deadline - time.perf_counter()
    if remaining <= 0:
        print("error: no SLA time remains before building workspace tests", flush=True)
        raise SystemExit(1)

    print(
        "building workspace libtest binaries: " + " ".join(WORKSPACE_TEST_BUILD_COMMAND),
        flush=True,
    )
    try:
        build = subprocess.run(
            WORKSPACE_TEST_BUILD_COMMAND,
            cwd=workspace_root,
            check=False,
            stdout=subprocess.PIPE,
            text=True,
            timeout=remaining,
        )
    except subprocess.TimeoutExpired:
        print(
            f"error: workspace test build exceeded remaining suite SLA ({remaining:.2f}s)",
            flush=True,
        )
        raise SystemExit(1) from None
    if build.returncode != 0:
        raise SystemExit(build.returncode)

    package_roots = workspace_package_roots(workspace_root)
    binaries: list[tuple[str, Path, Path]] = []
    for line in build.stdout.splitlines():
        try:
            message = json.loads(line)
        except json.JSONDecodeError:
            continue
        if message.get("reason") != "compiler-artifact":
            continue
        executable = message.get("executable")
        profile = message.get("profile", {})
        if not executable or not profile.get("test"):
            continue
        package_id = message["package_id"]
        target_name = message["target"]["name"]
        package_root = package_roots[package_id]
        if not target_has_tests(message, package_root):
            continue
        binaries.append((target_name, Path(executable), package_root))
    return binaries


def test_binary_env(workspace_root: Path, binaries: list[tuple[str, Path, Path]]) -> dict[str, str]:
    env = os.environ.copy()
    library_paths = [str(rust_target_libdir(workspace_root))]
    library_paths.extend(str(executable.parent) for _, executable, _ in binaries)
    deduped_paths = list(dict.fromkeys(library_paths))
    joined = os.pathsep.join(deduped_paths)
    env["DYLD_FALLBACK_LIBRARY_PATH"] = joined
    env["DYLD_LIBRARY_PATH"] = joined
    env.setdefault("RUST_TEST_THREADS", "1")
    return env


def terminate_process_group(process: subprocess.Popen[str]) -> None:
    try:
        os.killpg(process.pid, signal.SIGTERM)
    except ProcessLookupError:
        return


def run_workspace_test_binaries(workspace_root: Path, deadline: float) -> int:
    binaries = discover_workspace_test_binaries(workspace_root, deadline)
    if not binaries:
        print("skipping workspace libtest binaries: no test binaries found", flush=True)
        return 0

    env = test_binary_env(workspace_root, binaries)
    jobs = max(1, int(os.environ.get("LEAVEN_TEST_BINARY_JOBS", "8")))
    print(
        f"running workspace libtest binaries: {len(binaries)} binaries, {jobs} jobs",
        flush=True,
    )
    queued = list(binaries)
    running: list[tuple[subprocess.Popen[str], str, float, tempfile._TemporaryFileWrapper[bytes]]] = []
    completed = 0
    while queued or running:
        while queued and len(running) < jobs:
            target_name, executable, package_root = queued.pop(0)
            stderr_file = tempfile.TemporaryFile()
            process = subprocess.Popen(
                [str(executable)],
                cwd=package_root,
                stdout=subprocess.DEVNULL,
                stderr=stderr_file,
                text=True,
                start_new_session=True,
                env=env,
            )
            running.append((process, target_name, time.perf_counter(), stderr_file))

        remaining = deadline - time.perf_counter()
        if remaining <= 0:
            slow_running = sorted(
                (
                    (time.perf_counter() - started_at, target_name)
                    for _, target_name, started_at, _ in running
                ),
                reverse=True,
            )
            running_summary = ", ".join(
                f"{name} {elapsed:.1f}s" for elapsed, name in slow_running[:8]
            )
            print(
                "error: workspace libtest binaries exceeded suite SLA "
                f"after {completed}/{len(binaries)} binaries; "
                f"{len(running)} running and {len(queued)} queued",
                flush=True,
            )
            if running_summary:
                print(f"slowest running binaries: {running_summary}", flush=True)
            for process, _, _, stderr_file in running:
                terminate_process_group(process)
                stderr_file.close()
            return 1

        for item in list(running):
            process, target_name, started, stderr_file = item
            returncode = process.poll()
            if returncode is None:
                continue
            process.wait()
            stderr_file.seek(0)
            stderr = stderr_file.read().decode("utf-8", errors="replace")
            stderr_file.close()
            running.remove(item)
            completed += 1
            if returncode != 0:
                elapsed = time.perf_counter() - started
                sys.stderr.write(stderr)
                print(
                    f"error: test binary {target_name} failed with exit code "
                    f"{returncode} after {elapsed:.2f}s",
                    flush=True,
                )
                for other, _, _, other_stderr_file in running:
                    terminate_process_group(other)
                    other_stderr_file.close()
                return returncode
        time.sleep(0.01)
    return 0


def run_with_deadline(label: str, command: list[str], cwd: Path, deadline: float) -> int:
    remaining = deadline - time.perf_counter()
    if remaining <= 0:
        print(f"error: no SLA time remains before starting {label}", flush=True)
        return 1

    process = subprocess.Popen(command, cwd=cwd, start_new_session=True)
    try:
        return process.wait(timeout=remaining)
    except subprocess.TimeoutExpired:
        print(
            f"error: {label} exceeded remaining suite SLA "
            f"({remaining:.2f}s); terminating command",
            flush=True,
        )
        try:
            os.killpg(process.pid, signal.SIGTERM)
        except ProcessLookupError:
            return process.wait()
        try:
            process.wait(timeout=2.0)
        except subprocess.TimeoutExpired:
            try:
                os.killpg(process.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            process.wait()
        return 1


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
    deadline = started + args.sla_seconds
    workspace_root = Path.cwd()

    returncode = run_workspace_test_binaries(workspace_root, deadline)
    if returncode != 0:
        return returncode

    for label, command in test_commands(workspace_root):
        print(f"running {label}: {' '.join(command)}", flush=True)
        returncode = run_with_deadline(label, command, workspace_root, deadline)
        if returncode != 0:
            return returncode

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
