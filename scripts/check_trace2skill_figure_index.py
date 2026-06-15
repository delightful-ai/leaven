#!/usr/bin/env python3
"""Check Trace2Skill ARA figure evidence against upstream source files."""

from __future__ import annotations

import argparse
import hashlib
import re
import sys
from dataclasses import dataclass
from pathlib import Path


@dataclass(frozen=True)
class FigureSpec:
    markdown: str
    source_identifier: str
    source_file: str
    bytes_size: int
    sha256: str
    magic: bytes


FIGURE_SPECS = [
    FigureSpec(
        markdown="figure_trace2skill_framework.md",
        source_identifier="fig:pipeline",
        source_file="tmp/skill_opt_sources/arx_2603.25158/src/figures/trace2skill_framwork.png",
        bytes_size=379440,
        sha256="4ec0830d426a92198fb5b028a93a908de77787d983bee2f53813fb784f54845c",
        magic=b"\x89PNG\r\n\x1a\n",
    ),
]


def repo_root_for(ara_root: Path) -> Path:
    for candidate in (ara_root, *ara_root.parents):
        if (candidate / "tmp/skill_opt_sources/arx_2603.25158/src/figures").is_dir():
            return candidate
    return Path.cwd()


def markdown_rows(path: Path) -> dict[str, str]:
    rows: dict[str, str] = {}
    for line in path.read_text(encoding="utf-8").splitlines():
        if not line.startswith("|") or line.startswith("|-"):
            continue
        cells = [cell.strip() for cell in line.strip().strip("|").split("|")]
        if len(cells) != 2 or cells[0] == "Field":
            continue
        rows[cells[0]] = cells[1]
    return rows


def unquote_code_cell(cell: str) -> str:
    match = re.fullmatch(r"`([^`]+)`", cell.strip())
    return match.group(1) if match else cell.strip()


def check_figure_index(repo_root: Path, ara_root: Path) -> list[str]:
    errors: list[str] = []
    evidence_dir = ara_root / "evidence/figures"
    for spec in FIGURE_SPECS:
        markdown_path = evidence_dir / spec.markdown
        source_path = repo_root / spec.source_file
        if not markdown_path.is_file():
            errors.append(f"missing ARA figure evidence: {markdown_path}")
            continue
        if not source_path.is_file():
            errors.append(f"missing paper source figure: {source_path}")
            continue

        rows = markdown_rows(markdown_path)
        if unquote_code_cell(rows.get("Source identifier", "")) != spec.source_identifier:
            errors.append(f"{spec.markdown} Source identifier does not match {spec.source_identifier}")
        if unquote_code_cell(rows.get("Source file", "")) != spec.source_file:
            errors.append(f"{spec.markdown} Source file does not match {spec.source_file}")
        if unquote_code_cell(rows.get("Source SHA-256", "")) != spec.sha256:
            errors.append(f"{spec.markdown} Source SHA-256 does not match {spec.sha256}")
        if rows.get("Source bytes", "").strip() != str(spec.bytes_size):
            errors.append(f"{spec.markdown} Source bytes does not match {spec.bytes_size}")

        payload = source_path.read_bytes()
        if len(payload) != spec.bytes_size:
            errors.append(f"{spec.source_file} byte size {len(payload)} does not match {spec.bytes_size}")
        actual_sha = hashlib.sha256(payload).hexdigest()
        if actual_sha != spec.sha256:
            errors.append(f"{spec.source_file} SHA-256 {actual_sha} does not match {spec.sha256}")
        if not payload.startswith(spec.magic):
            errors.append(f"{spec.source_file} does not start with the expected PNG magic bytes")
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
    errors = check_figure_index(repo_root, ara_root)
    if errors:
        for error in errors:
            print(f"ERROR: {error}", file=sys.stderr)
        return 1
    print(f"PASS: {args.ara_dir} figure index ({len(FIGURE_SPECS)} figure)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
