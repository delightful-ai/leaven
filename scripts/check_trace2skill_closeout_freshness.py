#!/usr/bin/env python3
"""Validate that the Trace2Skill closeout audit is freshly generated."""

from __future__ import annotations

import argparse
import importlib.util
import json
import sys
import tempfile
from pathlib import Path
from typing import Any


def repo_root_for(ara_root: Path) -> Path:
    for candidate in (ara_root, *ara_root.parents):
        if (candidate / "docs/ara/trace2skill_spreadsheetbench").is_dir():
            return candidate
    return Path.cwd()


def import_closeout_auditor(repo_root: Path) -> Any:
    path = repo_root / "scripts/audit_trace2skill_closeout.py"
    spec = importlib.util.spec_from_file_location("audit_trace2skill_closeout", path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot import {path}")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def read(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def expected_outputs(repo_root: Path, ara_root: Path) -> tuple[str, str]:
    auditor = import_closeout_auditor(repo_root)
    report = auditor.audit(repo_root, ara_root)
    expected_json = json.dumps(report, indent=2, sort_keys=True) + "\n"
    with tempfile.TemporaryDirectory() as tmpdir:
        output_md = Path(tmpdir) / "closeout_audit.md"
        auditor.write_markdown(report, output_md)
        expected_md = read(output_md)
    return expected_json, expected_md


def check_closeout_freshness(repo_root: Path, ara_root: Path) -> list[str]:
    errors: list[str] = []
    expected_json, expected_md = expected_outputs(repo_root, ara_root)
    actual_json_path = ara_root / "results/closeout_audit.json"
    actual_md_path = ara_root / "results/closeout_audit.md"

    if not actual_json_path.is_file():
        errors.append("missing results/closeout_audit.json")
    elif read(actual_json_path) != expected_json:
        errors.append(
            "results/closeout_audit.json is stale; rerun "
            "`uv run --with pyyaml python scripts/audit_trace2skill_closeout.py "
            "docs/ara/trace2skill_spreadsheetbench`"
        )

    if not actual_md_path.is_file():
        errors.append("missing results/closeout_audit.md")
    elif read(actual_md_path) != expected_md:
        errors.append(
            "results/closeout_audit.md is stale; rerun "
            "`uv run --with pyyaml python scripts/audit_trace2skill_closeout.py "
            "docs/ara/trace2skill_spreadsheetbench`"
        )

    return errors


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("ara_dir", type=Path)
    args = parser.parse_args()
    ara_root = args.ara_dir.resolve()
    repo_root = repo_root_for(ara_root)

    errors = check_closeout_freshness(repo_root, ara_root)
    if errors:
        for error in errors:
            print(f"FAIL: {error}")
        return 1
    print(f"PASS: {args.ara_dir} closeout freshness")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
