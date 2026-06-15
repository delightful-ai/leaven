#!/usr/bin/env python3
"""Generate or verify the Trace2Skill prompt-source manifest."""

from __future__ import annotations

import argparse
import hashlib
import json
from dataclasses import dataclass
from pathlib import Path
from typing import Any


SCHEMA_VERSION = "leaven.trace2skill.prompt_manifest.v1"


@dataclass(frozen=True)
class PromptFamilySpec:
    family: str
    source_dir: str
    glob: str


PROMPT_FAMILIES = [
    PromptFamilySpec("Spreadsheet agent system prompts", "spreadsheet_agent/system_prompt", "*.txt"),
    PromptFamilySpec("Error evolving agent", "skill_evolver/prompts/skill_evolving_agent", "*.txt"),
    PromptFamilySpec("Success / combined evolving agent", "skill_evolver/prompts/success_evolving_agent", "*.txt"),
    PromptFamilySpec("Parallel merge/application agent", "skill_evolver/prompts/parallel_evolving_agent", "*.txt"),
    PromptFamilySpec("Released skill prompts", "released_skills", "*/SKILL.md"),
]


def repo_root_for(ara_root: Path) -> Path:
    for candidate in (ara_root, *ara_root.parents):
        if (candidate / "tmp/repros/trace2skill-upstream").is_dir():
            return candidate
    return Path.cwd()


def file_record(upstream_root: Path, path: Path) -> dict[str, Any]:
    data = path.read_bytes()
    rel_path = path.relative_to(upstream_root).as_posix()
    line_count = path.read_text(encoding="utf-8").count("\n")
    return {
        "path": rel_path,
        "bytes": len(data),
        "sha256": hashlib.sha256(data).hexdigest(),
        "line_count": line_count,
    }


def build_manifest(repo_root: Path) -> dict[str, Any]:
    upstream_root = repo_root / "tmp/repros/trace2skill-upstream"
    families = []
    total_files = 0
    for spec in PROMPT_FAMILIES:
        files = sorted((upstream_root / spec.source_dir).glob(spec.glob))
        total_files += len(files)
        families.append(
            {
                "family": spec.family,
                "source_dir": spec.source_dir,
                "glob": spec.glob,
                "file_count": len(files),
                "files": [file_record(upstream_root, path) for path in files],
            }
        )
    return {
        "schema_version": SCHEMA_VERSION,
        "source_root": "tmp/repros/trace2skill-upstream",
        "generated_by": "scripts/check_trace2skill_prompt_manifest.py --write",
        "total_files": total_files,
        "families": families,
    }


def load_manifest(path: Path) -> dict[str, Any]:
    loaded = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(loaded, dict):
        raise ValueError(f"{path} must contain a JSON object")
    return loaded


def check_prompt_manifest(repo_root: Path, ara_root: Path) -> list[str]:
    manifest_path = ara_root / "evidence/prompt_templates.manifest.json"
    if not manifest_path.is_file():
        return ["missing evidence/prompt_templates.manifest.json"]
    expected = build_manifest(repo_root)
    actual = load_manifest(manifest_path)
    if actual != expected:
        return ["evidence/prompt_templates.manifest.json is stale; rerun with --write"]
    return []


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("ara_dir", type=Path)
    parser.add_argument("--write", action="store_true")
    args = parser.parse_args()

    ara_root = args.ara_dir.resolve()
    repo_root = repo_root_for(ara_root)
    manifest_path = ara_root / "evidence/prompt_templates.manifest.json"

    if args.write:
        manifest = build_manifest(repo_root)
        manifest_path.write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n", encoding="utf-8")
        print(f"wrote {manifest_path.relative_to(repo_root)}")
        return 0

    errors = check_prompt_manifest(repo_root, ara_root)
    if errors:
        for error in errors:
            print(f"FAIL: {error}")
        return 1
    print(f"PASS: {args.ara_dir} prompt manifest")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
