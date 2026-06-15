#!/usr/bin/env python3
"""Check Trace2Skill ARA prompt index against the upstream checkout."""

from __future__ import annotations

import argparse
import re
import sys
from dataclasses import dataclass
from pathlib import Path


@dataclass(frozen=True)
class PromptFamilySpec:
    family: str
    source_dir: str
    glob: str
    count: int


PROMPT_FAMILIES = [
    PromptFamilySpec("Spreadsheet agent system prompts", "spreadsheet_agent/system_prompt", "*.txt", 2),
    PromptFamilySpec("Error evolving agent", "skill_evolver/prompts/skill_evolving_agent", "*.txt", 18),
    PromptFamilySpec("Success / combined evolving agent", "skill_evolver/prompts/success_evolving_agent", "*.txt", 43),
    PromptFamilySpec("Parallel merge/application agent", "skill_evolver/prompts/parallel_evolving_agent", "*.txt", 36),
    PromptFamilySpec("Released skill prompts", "released_skills", "*/SKILL.md", 4),
]


def repo_root_for(ara_root: Path) -> Path:
    for candidate in (ara_root, *ara_root.parents):
        if (candidate / "tmp/repros/trace2skill-upstream").is_dir():
            return candidate
    return Path.cwd()


def markdown_rows(path: Path) -> dict[str, list[str]]:
    rows: dict[str, list[str]] = {}
    for line in path.read_text(encoding="utf-8").splitlines():
        if not line.startswith("|") or line.startswith("|-"):
            continue
        cells = [cell.strip() for cell in line.strip().strip("|").split("|")]
        if not cells or cells[0] == "Prompt family":
            continue
        rows[cells[0]] = cells
    return rows


def numeric_count(cell: str) -> int | None:
    match = re.match(r"^(\d+)", cell.strip())
    return int(match.group(1)) if match else None


def representative_paths(cell: str) -> list[str]:
    return re.findall(r"`([^`]+)`", cell)


def check_prompt_index(repo_root: Path, ara_root: Path) -> list[str]:
    errors: list[str] = []
    upstream_root = repo_root / "tmp/repros/trace2skill-upstream"
    index_path = ara_root / "evidence/prompt_templates.md"
    if not index_path.is_file():
        return [f"missing prompt index: {index_path}"]
    if not upstream_root.is_dir():
        return [f"missing upstream Trace2Skill checkout: {upstream_root}"]

    rows = markdown_rows(index_path)
    for spec in PROMPT_FAMILIES:
        row = rows.get(spec.family)
        if row is None:
            errors.append(f"prompt_templates.md missing family row: {spec.family}")
            continue
        if len(row) < 5:
            errors.append(f"prompt_templates.md row for {spec.family} has too few columns")
            continue

        indexed_count = numeric_count(row[1])
        if indexed_count != spec.count:
            errors.append(
                f"{spec.family} indexed count {indexed_count!r} does not match expected {spec.count}"
            )

        actual_files = sorted((upstream_root / spec.source_dir).glob(spec.glob))
        if len(actual_files) != spec.count:
            errors.append(
                f"{spec.source_dir}/{spec.glob} actual count {len(actual_files)} does not match {spec.count}"
            )

        for rel_path in representative_paths(row[2]):
            if not (upstream_root / rel_path).is_file():
                errors.append(f"{spec.family} representative path is not inspectable: {rel_path}")
    return errors


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "ara_dir",
        type=Path,
        default=Path("docs/ara/trace2skill_spreadsheetbench"),
        nargs="?",
    )
    args = parser.parse_args()

    ara_root = args.ara_dir.resolve()
    repo_root = repo_root_for(ara_root)
    errors = check_prompt_index(repo_root, ara_root)
    if errors:
        for error in errors:
            print(f"ERROR: {error}", file=sys.stderr)
        return 1
    print(f"PASS: {args.ara_dir} prompt index ({len(PROMPT_FAMILIES)} families)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
