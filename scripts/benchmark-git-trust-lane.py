#!/usr/bin/env python3
"""Benchmark the local GitProgram trust lane.

This script is intentionally local-only: it creates synthetic Git repositories,
exercises the same projection/materialization/readback shape as the Leaven Git
adapters, and writes a JSON report under target/ by default.
"""

from __future__ import annotations

import argparse
import concurrent.futures
import dataclasses
import datetime as dt
import hashlib
import json
import os
import platform
import resource
import shutil
import statistics
import subprocess
import sys
import tempfile
import time
from pathlib import Path


@dataclasses.dataclass(frozen=True)
class BenchCase:
    name: str
    file_count: int
    file_bytes: int

    @property
    def total_bytes(self) -> int:
        return self.file_count * self.file_bytes


@dataclasses.dataclass(frozen=True)
class CommandResult:
    command: list[str]
    seconds: float


@dataclasses.dataclass(frozen=True)
class BenchResult:
    case: BenchCase
    iteration: int
    setup_seconds: float
    projection_seconds: float
    materialize_seconds: float
    readback_seconds: float
    durable_kib: int
    workspace_kib: int
    imported_child: str


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--case",
        action="append",
        default=[],
        metavar="NAME:FILES:BYTES",
        help="Benchmark case. May be repeated.",
    )
    parser.add_argument(
        "--iterations",
        type=int,
        default=3,
        help="Iterations per case. Default: 3.",
    )
    parser.add_argument(
        "--jobs",
        type=int,
        default=max(1, (os.cpu_count() or 2) // 2),
        help="Parallel workers. Default: half logical CPUs.",
    )
    parser.add_argument(
        "--out",
        type=Path,
        default=None,
        help="Report path. Default: target/git-trust-lane/<timestamp>.json.",
    )
    parser.add_argument(
        "--skip-trust-tests",
        action="store_true",
        help="Skip the cargo trust tests and run only synthetic benchmarks.",
    )
    parser.add_argument(
        "--keep-workdirs",
        action="store_true",
        help="Keep generated benchmark work directories under target/git-trust-lane/work.",
    )
    parser.add_argument(
        "--verbose",
        action="store_true",
        help="Print every git/cargo command.",
    )
    args = parser.parse_args()

    if args.iterations < 1:
        parser.error("--iterations must be positive")
    if args.jobs < 1:
        parser.error("--jobs must be positive")

    repo = repo_root()
    cases = parse_cases(args.case)
    stamp = dt.datetime.now(dt.UTC).strftime("%Y%m%dT%H%M%SZ")
    report_path = args.out or repo / "target" / "git-trust-lane" / f"{stamp}.json"
    work_root = report_path.parent / "work" / stamp
    report_path.parent.mkdir(parents=True, exist_ok=True)
    work_root.mkdir(parents=True, exist_ok=True)

    print(f"repo: {repo}")
    print(f"jobs: {args.jobs} of {os.cpu_count() or 'unknown'} logical CPUs")
    print(f"cases: {', '.join(format_case(case) for case in cases)}")
    print(f"iterations: {args.iterations}")
    print(f"report: {report_path}")

    command_results: list[CommandResult] = []
    if not args.skip_trust_tests:
        print("trust tests: cargo test -p leaven-workspace-git --test git_projection")
        command_results.append(
            timed_command(
                [
                    "cargo",
                    "test",
                    "-p",
                    "leaven-workspace-git",
                    "--test",
                    "git_projection",
                ],
                repo,
                args.jobs,
                args.verbose,
            )
        )
        print("trust tests: cargo test -p leaven-agentic-git --test git_program_materializer")
        command_results.append(
            timed_command(
                [
                    "cargo",
                    "test",
                    "-p",
                    "leaven-agentic-git",
                    "--test",
                    "git_program_materializer",
                ],
                repo,
                args.jobs,
                args.verbose,
            )
        )

    jobs = [
        (case, iteration)
        for case in cases
        for iteration in range(1, args.iterations + 1)
    ]
    print(f"synthetic benchmark: {len(jobs)} local GitProgram cycles")
    start_usage = resource.getrusage(resource.RUSAGE_CHILDREN)
    started = time.perf_counter()
    results: list[BenchResult] = []
    with concurrent.futures.ThreadPoolExecutor(max_workers=args.jobs) as executor:
        futures = [
            executor.submit(run_bench_cycle, repo, work_root, case, iteration, args.verbose)
            for case, iteration in jobs
        ]
        for future in concurrent.futures.as_completed(futures):
            result = future.result()
            results.append(result)
            print(
                "done "
                f"{result.case.name}#{result.iteration}: "
                f"setup={result.setup_seconds:.3f}s "
                f"project={result.projection_seconds:.3f}s "
                f"materialize={result.materialize_seconds:.3f}s "
                f"readback={result.readback_seconds:.3f}s"
            )
    elapsed = time.perf_counter() - started
    end_usage = resource.getrusage(resource.RUSAGE_CHILDREN)

    report = {
        "generated_at": stamp,
        "host": {
            "system": platform.platform(),
            "logical_cpus": os.cpu_count(),
            "jobs": args.jobs,
            "python": sys.version,
        },
        "commands": [
            {"command": item.command, "seconds": item.seconds}
            for item in command_results
        ],
        "cases": [
            dataclasses.asdict(case)
            for case in cases
        ],
        "results": [
            {
                "case": dataclasses.asdict(result.case),
                "iteration": result.iteration,
                "setup_seconds": result.setup_seconds,
                "projection_seconds": result.projection_seconds,
                "materialize_seconds": result.materialize_seconds,
                "readback_seconds": result.readback_seconds,
                "durable_kib": result.durable_kib,
                "workspace_kib": result.workspace_kib,
                "imported_child": result.imported_child,
            }
            for result in sorted(results, key=lambda r: (r.case.name, r.iteration))
        ],
        "summary": summarize(results),
        "resource_usage": {
            "wall_seconds": elapsed,
            "child_user_seconds": end_usage.ru_utime - start_usage.ru_utime,
            "child_system_seconds": end_usage.ru_stime - start_usage.ru_stime,
            "child_maxrss": end_usage.ru_maxrss,
        },
    }
    report_path.write_text(json.dumps(report, indent=2) + "\n")
    print_summary(report["summary"])

    if not args.keep_workdirs:
        shutil.rmtree(work_root, ignore_errors=True)
    else:
        print(f"kept workdirs: {work_root}")
    print(f"wrote report: {report_path}")
    return 0


def repo_root() -> Path:
    return Path(__file__).resolve().parents[1]


def parse_cases(raw_cases: list[str]) -> list[BenchCase]:
    if not raw_cases:
        raw_cases = [
            "small:100:1024",
            "medium:1000:4096",
            "large:5000:4096",
        ]
    cases = []
    for raw in raw_cases:
        parts = raw.split(":")
        if len(parts) != 3:
            raise SystemExit(f"invalid --case {raw!r}; expected NAME:FILES:BYTES")
        name, files, bytes_per_file = parts
        cases.append(BenchCase(name, int(files), int(bytes_per_file)))
    return cases


def format_case(case: BenchCase) -> str:
    mib = case.total_bytes / (1024 * 1024)
    return f"{case.name}={case.file_count}x{case.file_bytes}B ({mib:.1f} MiB)"


def timed_command(command: list[str], cwd: Path, jobs: int, verbose: bool) -> CommandResult:
    env = os.environ.copy()
    env["CARGO_BUILD_JOBS"] = str(jobs)
    env.setdefault("NEXTEST_TEST_THREADS", str(jobs))
    before = time.perf_counter()
    run(command, cwd=cwd, env=env, verbose=verbose)
    return CommandResult(command=command, seconds=time.perf_counter() - before)


def run_bench_cycle(
    repo: Path,
    work_root: Path,
    case: BenchCase,
    iteration: int,
    verbose: bool,
) -> BenchResult:
    root = work_root / f"{case.name}-{iteration}"
    root.mkdir(parents=True, exist_ok=True)
    source = root / "source"
    durable = root / "program.git"
    workspace = root / "workspace"
    projection = root / "archive.git"

    before = time.perf_counter()
    create_source_repo(source, case, verbose)
    run(
        ["git", "clone", "--bare", "--no-local", str(source), str(durable)],
        cwd=repo,
        verbose=verbose,
    )
    parent = git_output(["git", "rev-parse", "main"], cwd=source, verbose=verbose).strip()
    hidden = git_output(
        ["git", "rev-parse", "refs/heads/hidden/eval"],
        cwd=source,
        verbose=verbose,
    ).strip()
    setup_seconds = time.perf_counter() - before

    before = time.perf_counter()
    create_projection(source, projection, hidden, verbose)
    projection_seconds = time.perf_counter() - before

    before = time.perf_counter()
    checkout = materialize_program(durable, workspace, parent, verbose)
    materialize_seconds = time.perf_counter() - before

    before = time.perf_counter()
    child = mutate_and_import_child(durable, checkout, parent, verbose)
    readback_seconds = time.perf_counter() - before

    return BenchResult(
        case=case,
        iteration=iteration,
        setup_seconds=setup_seconds,
        projection_seconds=projection_seconds,
        materialize_seconds=materialize_seconds,
        readback_seconds=readback_seconds,
        durable_kib=du_kib(durable),
        workspace_kib=du_kib(workspace),
        imported_child=child,
    )


def create_source_repo(source: Path, case: BenchCase, verbose: bool) -> None:
    source.mkdir(parents=True, exist_ok=True)
    run(["git", "init", "--initial-branch=main"], cwd=source, verbose=verbose)
    run(["git", "config", "user.name", "Leaven Benchmark"], cwd=source, verbose=verbose)
    run(
        ["git", "config", "user.email", "leaven@example.invalid"],
        cwd=source,
        verbose=verbose,
    )
    data_dir = source / "src"
    data_dir.mkdir()
    for index in range(case.file_count):
        write_payload(data_dir / f"file-{index:05d}.dat", case.file_bytes, case.name, index)
    run(["git", "add", "src"], cwd=source, verbose=verbose)
    run(["git", "commit", "-m", "base"], cwd=source, verbose=verbose)
    run(["git", "checkout", "-b", "hidden/eval"], cwd=source, verbose=verbose)
    write_payload(source / "hidden-evaluator-target.dat", 8192, case.name, 999_999)
    run(["git", "add", "hidden-evaluator-target.dat"], cwd=source, verbose=verbose)
    run(["git", "commit", "-m", "hidden evaluator target"], cwd=source, verbose=verbose)
    run(["git", "checkout", "main"], cwd=source, verbose=verbose)


def create_projection(source: Path, projection: Path, hidden_commit: str, verbose: bool) -> None:
    run(["git", "init", "--bare", str(projection)], verbose=verbose)
    run(
        [
            "git",
            "fetch",
            str(source),
            "+refs/heads/main:refs/heads/program/base",
        ],
        cwd=projection,
        verbose=verbose,
    )
    run(["git", "fsck", "--strict"], cwd=projection, verbose=verbose)
    assert_fails(
        ["git", "show-ref", "--verify", "refs/heads/hidden/eval"],
        cwd=projection,
        verbose=verbose,
    )
    assert_fails(["git", "cat-file", "-e", hidden_commit], cwd=projection, verbose=verbose)
    alternates = projection / "objects" / "info" / "alternates"
    if alternates.exists():
        raise RuntimeError(f"projection leaked alternates file: {alternates}")


def materialize_program(durable: Path, workspace: Path, parent: str, verbose: bool) -> Path:
    checkout = workspace / "repos" / "program"
    checkout.parent.mkdir(parents=True, exist_ok=True)
    bundle = workspace / "materialization.bundle"
    temp_ref = f"refs/leaven/materialize/{parent}"
    run(["git", "update-ref", temp_ref, parent], cwd=durable, verbose=verbose)
    try:
        run(["git", "bundle", "create", str(bundle), temp_ref], cwd=durable, verbose=verbose)
    finally:
        run(["git", "update-ref", "-d", temp_ref], cwd=durable, verbose=verbose)
    run(["git", "init", str(checkout)], verbose=verbose)
    materialized = f"refs/leaven/materialized/{parent}"
    run(
        [
            "git",
            "fetch",
            "--no-tags",
            "--no-write-fetch-head",
            str(bundle),
            f"+{parent}:{materialized}",
        ],
        cwd=checkout,
        verbose=verbose,
    )
    bundle.unlink()
    run(["git", "checkout", "--detach", parent], cwd=checkout, verbose=verbose)
    run(["git", "ls-files", "-z"], cwd=checkout, verbose=verbose)
    return checkout


def mutate_and_import_child(durable: Path, checkout: Path, parent: str, verbose: bool) -> str:
    run(["git", "config", "user.name", "Leaven Benchmark"], cwd=checkout, verbose=verbose)
    run(
        ["git", "config", "user.email", "leaven@example.invalid"],
        cwd=checkout,
        verbose=verbose,
    )
    target = checkout / "src" / "file-00000.dat"
    with target.open("ab") as handle:
        handle.write(b"\nleaven benchmark child mutation\n")
    run(["git", "add", "-A"], cwd=checkout, verbose=verbose)
    run(["git", "commit", "-m", "leaven workspace snapshot"], cwd=checkout, verbose=verbose)
    child = git_output(["git", "rev-parse", "HEAD"], cwd=checkout, verbose=verbose).strip()
    bundle = checkout / ".git" / "leaven-readback.bundle"
    run(
        ["git", "bundle", "create", str(bundle), "HEAD", f"^{parent}"],
        cwd=checkout,
        verbose=verbose,
    )
    imported = import_bundle(durable, bundle, parent, child, verbose)
    bundle.unlink()
    return imported


def import_bundle(
    durable: Path,
    bundle: Path,
    parent: str,
    child: str,
    verbose: bool,
) -> str:
    with tempfile.TemporaryDirectory(prefix="leaven-git-bundle-") as temp_name:
        temp = Path(temp_name)
        run(["git", "init", "--bare", str(temp)], verbose=verbose)
        run(
            ["git", "fetch", str(durable), f"+{parent}:refs/leaven/parents/{parent}"],
            cwd=temp,
            verbose=verbose,
        )
        run(["git", "bundle", "verify", str(bundle)], cwd=temp, verbose=verbose)
        run(
            ["git", "fetch", str(bundle), f"+{child}:refs/leaven/proposals/{child}"],
            cwd=temp,
            verbose=verbose,
        )
        run(["git", "fsck", "--strict"], cwd=temp, verbose=verbose)
        run(["git", "merge-base", "--is-ancestor", parent, child], cwd=temp, verbose=verbose)
        run(
            ["git", "fetch", str(temp), f"+{child}:refs/leaven/imported/{child}"],
            cwd=durable,
            verbose=verbose,
        )
    run(["git", "fsck", "--strict"], cwd=durable, verbose=verbose)
    return child


def write_payload(path: Path, size: int, case_name: str, index: int) -> None:
    seed = f"{case_name}:{index:05d}:".encode()
    counter = 0
    remaining = size
    with path.open("wb") as handle:
        while remaining > 0:
            chunk = hashlib.sha256(seed + counter.to_bytes(8, "little")).digest()
            counter += 1
            take = min(remaining, len(chunk))
            handle.write(chunk[:take])
            remaining -= take


def du_kib(path: Path) -> int:
    output = subprocess.run(
        ["du", "-sk", str(path)],
        check=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    ).stdout
    return int(output.split()[0])


def git_output(command: list[str], cwd: Path, verbose: bool) -> str:
    completed = run(command, cwd=cwd, verbose=verbose, capture=True)
    return completed.stdout


def run(
    command: list[str],
    cwd: Path | None = None,
    env: dict[str, str] | None = None,
    verbose: bool = False,
    capture: bool = False,
) -> subprocess.CompletedProcess[str]:
    if verbose:
        prefix = f"(cd {cwd} && " if cwd else ""
        suffix = ")" if cwd else ""
        print(prefix + " ".join(command) + suffix)
    fixed_env = os.environ.copy()
    fixed_env.update(
        {
            "GIT_AUTHOR_NAME": "Leaven Benchmark",
            "GIT_AUTHOR_EMAIL": "leaven@example.invalid",
            "GIT_COMMITTER_NAME": "Leaven Benchmark",
            "GIT_COMMITTER_EMAIL": "leaven@example.invalid",
            "GIT_AUTHOR_DATE": "2026-01-01T00:00:00Z",
            "GIT_COMMITTER_DATE": "2026-01-01T00:00:00Z",
        }
    )
    if env:
        fixed_env.update(env)
    completed = subprocess.run(
        command,
        cwd=cwd,
        env=fixed_env,
        stdout=subprocess.PIPE if capture or not verbose else None,
        stderr=subprocess.PIPE if capture or not verbose else None,
        text=True,
    )
    if completed.returncode != 0:
        if completed.stdout:
            print(completed.stdout, file=sys.stdout)
        if completed.stderr:
            print(completed.stderr, file=sys.stderr)
        raise subprocess.CalledProcessError(
            completed.returncode,
            command,
            output=completed.stdout,
            stderr=completed.stderr,
        )
    return completed


def assert_fails(command: list[str], cwd: Path, verbose: bool) -> None:
    completed = subprocess.run(
        command,
        cwd=cwd,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    if verbose:
        print(f"expected failure {command}: status={completed.returncode}")
    if completed.returncode == 0:
        raise RuntimeError(f"expected command to fail: {command}")


def summarize(results: list[BenchResult]) -> dict[str, dict[str, float]]:
    by_case: dict[str, list[BenchResult]] = {}
    for result in results:
        by_case.setdefault(result.case.name, []).append(result)
    summary: dict[str, dict[str, float]] = {}
    for name, values in sorted(by_case.items()):
        summary[name] = {
            "file_count": values[0].case.file_count,
            "file_bytes": values[0].case.file_bytes,
            "total_mib": values[0].case.total_bytes / (1024 * 1024),
            "setup_mean_seconds": mean(v.setup_seconds for v in values),
            "projection_mean_seconds": mean(v.projection_seconds for v in values),
            "materialize_mean_seconds": mean(v.materialize_seconds for v in values),
            "readback_mean_seconds": mean(v.readback_seconds for v in values),
            "durable_kib_mean": mean(v.durable_kib for v in values),
            "workspace_kib_mean": mean(v.workspace_kib for v in values),
        }
    return summary


def mean(values: object) -> float:
    return float(statistics.fmean(list(values)))


def print_summary(summary: dict[str, dict[str, float]]) -> None:
    print("summary:")
    print(
        "case,total_mib,project_mean_s,materialize_mean_s,readback_mean_s,"
        "durable_kib,workspace_kib"
    )
    for name, values in summary.items():
        print(
            f"{name},{values['total_mib']:.2f},"
            f"{values['projection_mean_seconds']:.3f},"
            f"{values['materialize_mean_seconds']:.3f},"
            f"{values['readback_mean_seconds']:.3f},"
            f"{values['durable_kib_mean']:.0f},"
            f"{values['workspace_kib_mean']:.0f}"
        )


if __name__ == "__main__":
    raise SystemExit(main())
