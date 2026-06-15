#!/usr/bin/env python3
"""Generate or verify Trace2Skill upstream execution-code manifest."""

from __future__ import annotations

import argparse
import hashlib
import json
from dataclasses import dataclass
from pathlib import Path
from typing import Any


SCHEMA_VERSION = "leaven.trace2skill.upstream_code_manifest.v1"


@dataclass(frozen=True)
class UpstreamCodeFile:
    path: str
    role: str


UPSTREAM_CODE_FILES = [
    UpstreamCodeFile("run_spreadsheetbench.py", "SpreadsheetBench agent execution entrypoint."),
    UpstreamCodeFile("evaluate_with_official.py", "Official SpreadsheetBench evaluator entrypoint."),
    UpstreamCodeFile("analyze_results.py", "Trajectory outcome analysis entrypoint."),
    UpstreamCodeFile("analysis/run_error_analysis.py", "Stage 2 error-analysis trajectory analyst entrypoint."),
    UpstreamCodeFile("analysis/run_success_analysis_llm.py", "Stage 2 success-analysis trajectory analyst entrypoint."),
    UpstreamCodeFile("analysis/parse_error_analysis_outputs.py", "Error-analysis parser entrypoint."),
    UpstreamCodeFile("analysis/parse_success_analysis_outputs.py", "Success-analysis parser entrypoint."),
    UpstreamCodeFile("skill_evolver/run_parallel_skill_evolution.py", "Stage 2/3 parallel patch generation and merge entrypoint."),
]


def repo_root_for(ara_root: Path) -> Path:
    for candidate in (ara_root, *ara_root.parents):
        if (candidate / "tmp/repros/trace2skill-upstream").is_dir():
            return candidate
    return Path.cwd()


def read(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def file_record(upstream_root: Path, file: UpstreamCodeFile) -> dict[str, Any]:
    path = upstream_root / file.path
    data = path.read_bytes()
    return {
        "path": file.path,
        "role": file.role,
        "bytes": len(data),
        "line_count": read(path).count("\n"),
        "sha256": hashlib.sha256(data).hexdigest(),
    }


def build_manifest(repo_root: Path) -> dict[str, Any]:
    upstream_root = repo_root / "tmp/repros/trace2skill-upstream"
    files = [file_record(upstream_root, file) for file in UPSTREAM_CODE_FILES]
    return {
        "schema_version": SCHEMA_VERSION,
        "source_root": "tmp/repros/trace2skill-upstream",
        "generated_by": "scripts/check_trace2skill_upstream_code_manifest.py --write",
        "file_count": len(files),
        "files": files,
    }


def load_manifest(path: Path) -> dict[str, Any]:
    loaded = json.loads(read(path))
    if not isinstance(loaded, dict):
        raise ValueError(f"{path} must contain a JSON object")
    return loaded


def check_runbook_references(ara_root: Path) -> list[str]:
    errors: list[str] = []
    runbook = json.loads(read(ara_root / "results/full_denominator_runbook.json"))
    command_text = "\n".join(
        command
        for stage in runbook.get("stages", [])
        if isinstance(stage, dict)
        for command in stage.get("commands", [])
        if isinstance(command, str)
    )
    required_snippets = {
        "run_spreadsheetbench.py": "run_spreadsheetbench.py",
        "evaluate_with_official.py": "evaluate_with_official.py",
        "analyze_results.py": "analyze_results.py",
        "analysis/run_error_analysis.py": "analysis/run_error_analysis.py",
        "analysis/run_success_analysis_llm.py": "analysis/run_success_analysis_llm.py",
        "analysis/parse_error_analysis_outputs.py": "analysis/parse_error_analysis_outputs.py",
        "analysis/parse_success_analysis_outputs.py": "analysis/parse_success_analysis_outputs.py",
        "skill_evolver/run_parallel_skill_evolution.py": "skill_evolver.run_parallel_skill_evolution",
    }
    for path, snippet in required_snippets.items():
        if snippet not in command_text:
            errors.append(f"full_denominator_runbook.json does not reference upstream code path/module {path}")
    return errors


def check_upstream_code_manifest(repo_root: Path, ara_root: Path) -> list[str]:
    manifest_path = ara_root / "src/execution/upstream_code_manifest.json"
    if not manifest_path.is_file():
        return ["missing src/execution/upstream_code_manifest.json"]
    expected = build_manifest(repo_root)
    actual = load_manifest(manifest_path)
    errors: list[str] = []
    if actual != expected:
        errors.append("src/execution/upstream_code_manifest.json is stale; rerun with --write")
    errors.extend(check_runbook_references(ara_root))
    return errors


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("ara_dir", type=Path)
    parser.add_argument("--write", action="store_true")
    args = parser.parse_args()

    ara_root = args.ara_dir.resolve()
    repo_root = repo_root_for(ara_root)
    manifest_path = ara_root / "src/execution/upstream_code_manifest.json"

    if args.write:
        manifest = build_manifest(repo_root)
        manifest_path.write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n", encoding="utf-8")
        print(f"wrote {manifest_path.relative_to(repo_root)}")
        return 0

    errors = check_upstream_code_manifest(repo_root, ara_root)
    if errors:
        for error in errors:
            print(f"FAIL: {error}")
        return 1
    print(f"PASS: {args.ara_dir} upstream code manifest")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
