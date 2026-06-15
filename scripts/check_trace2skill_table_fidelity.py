#!/usr/bin/env python3
"""Check Trace2Skill ARA evidence tables against the paper TeX sources."""

from __future__ import annotations

import argparse
import re
import sys
from dataclasses import dataclass
from pathlib import Path


@dataclass(frozen=True)
class TableSpec:
    ara_markdown: str
    source_tex: str
    markdown_label_columns: int


TABLE_SPECS = [
    TableSpec("table_agentic_ablation.md", "table_agentic_ablation.tex", 3),
    TableSpec("table_main_spreadsheetbench.md", "table_main_v1.tex", 3),
    TableSpec("table_math.md", "table_math.tex", 1),
    TableSpec("table_parallel_vs_sequential.md", "table_seq_parallel.tex", 1),
    TableSpec("table_reasoningbank.md", "table_reasoning_bank.tex", 1),
    TableSpec("table_vqa.md", "table_vqa.tex", 1),
]


def repo_root_for(ara_root: Path) -> Path:
    for candidate in (ara_root, *ara_root.parents):
        if (candidate / "tmp/skill_opt_sources/arx_2603.25158/src/tables").is_dir():
            return candidate
    return Path.cwd()


def normalize_markdown_cell(cell: str) -> str:
    return re.sub(r"\s+", " ", cell.strip().replace(r"\|", "|"))


def normalize_latex_cell(cell: str) -> str:
    cleaned = cell.strip()
    cleaned = re.sub(r"%.*", "", cleaned)
    cleaned = cleaned.replace(r"$\sim$", "~")
    cleaned = re.sub(r"\\cellcolor\{[^{}]*\}", "", cleaned)
    cleaned = re.sub(r"\\citep\{[^{}]*\}", "", cleaned)
    previous = None
    while previous != cleaned:
        previous = cleaned
        cleaned = re.sub(r"\\(?:textbf|textit|emph)\{([^{}]*)\}", r"\1", cleaned)
    cleaned = cleaned.replace("$", "")
    cleaned = cleaned.replace("{", "").replace("}", "")
    cleaned = cleaned.replace(r"\quad", "")
    cleaned = cleaned.replace(r"\,", ",")
    cleaned = cleaned.replace(r"\%", "%")
    cleaned = re.sub(r"\\[a-zA-Z]+", "", cleaned)
    cleaned = cleaned.replace("~ ", "~")
    cleaned = re.sub(r"\s+", " ", cleaned)
    return cleaned.strip()


def markdown_table_rows(path: Path, label_columns: int) -> list[list[str]]:
    lines = path.read_text(encoding="utf-8").splitlines()
    table_lines = [line for line in lines if line.startswith("|")]
    if len(table_lines) < 3:
        raise ValueError(f"{path} does not contain a Markdown table")

    body: list[list[str]] = []
    for line in table_lines[2:]:
        cells = [normalize_markdown_cell(cell) for cell in line.strip().strip("|").split("|")]
        if len(cells) <= label_columns:
            raise ValueError(f"{path} row has too few cells: {line}")
        body.append(cells[label_columns:])
    return body


def latex_table_rows(path: Path) -> list[list[str]]:
    rows: list[list[str]] = []
    in_body = False
    buffer = ""
    for raw_line in path.read_text(encoding="utf-8").splitlines():
        line = raw_line.strip()
        if not in_body:
            if line.startswith(r"\midrule"):
                in_body = True
            continue
        if line.startswith(r"\bottomrule"):
            break
        if not buffer and (
            not line
            or line.startswith("%")
            or line.startswith(r"\midrule")
            or line.startswith(r"\multicolumn")
        ):
            continue
        if "&" not in line and not buffer:
            continue

        buffer = f"{buffer} {line}".strip()
        if r"\\" not in line:
            continue

        row = buffer.split(r"\\", 1)[0]
        buffer = ""
        if "&" not in row:
            continue
        cells = [normalize_latex_cell(cell) for cell in row.split("&")]
        if len(cells) < 2:
            continue
        rows.append(cells[1:])
    return rows


def check_table_fidelity(repo_root: Path, ara_root: Path) -> list[str]:
    errors: list[str] = []
    evidence_dir = ara_root / "evidence/tables"
    source_dir = repo_root / "tmp/skill_opt_sources/arx_2603.25158/src/tables"
    for spec in TABLE_SPECS:
        markdown_path = evidence_dir / spec.ara_markdown
        source_path = source_dir / spec.source_tex
        if not markdown_path.is_file():
            errors.append(f"missing ARA evidence table: {markdown_path}")
            continue
        if not source_path.is_file():
            errors.append(f"missing paper source table: {source_path}")
            continue

        try:
            markdown_rows = markdown_table_rows(markdown_path, spec.markdown_label_columns)
            source_rows = latex_table_rows(source_path)
        except ValueError as exc:
            errors.append(str(exc))
            continue

        if len(markdown_rows) != len(source_rows):
            errors.append(
                f"{spec.ara_markdown} row count {len(markdown_rows)} does not match "
                f"{spec.source_tex} row count {len(source_rows)}"
            )
            continue

        for row_index, (markdown_row, source_row) in enumerate(zip(markdown_rows, source_rows, strict=True), start=1):
            if markdown_row != source_row:
                errors.append(
                    f"{spec.ara_markdown} row {row_index} cells {markdown_row!r} "
                    f"do not match {spec.source_tex} cells {source_row!r}"
                )
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
    errors = check_table_fidelity(repo_root, ara_root)
    if errors:
        for error in errors:
            print(f"ERROR: {error}", file=sys.stderr)
        return 1
    print(f"PASS: {args.ara_dir} table fidelity ({len(TABLE_SPECS)} tables)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
