#!/usr/bin/env python3
"""Check Trace2Skill target-plot provenance against ARA evidence and result rows."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import sys
from pathlib import Path
from typing import Any


SCHEMA_VERSION = "leaven.trace2skill.plot_provenance.v1"
PLOT_FILE = "plots/trace2skill_targets.png"
PROVENANCE_FILE = "plots/trace2skill_targets.provenance.json"
TARGET_TABLES = [
    "evidence/tables/table_main_spreadsheetbench.md",
    "evidence/tables/table_parallel_vs_sequential.md",
    "evidence/tables/table_reasoningbank.md",
]
PAPER_DENOMINATOR_CLASSIFICATIONS = {
    "paper-denominator-candidate",
    "paper-denominator-reproduction",
}


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def parse_markdown_table(path: Path) -> list[dict[str, str]]:
    lines = [line.strip() for line in path.read_text(encoding="utf-8").splitlines() if line.strip().startswith("|")]
    if len(lines) < 3:
        raise ValueError(f"{path} does not contain a Markdown table")
    header = split_row(lines[0])
    rows: list[dict[str, str]] = []
    for line in lines[2:]:
        cells = split_row(line)
        if len(cells) == len(header):
            rows.append(dict(zip(header, cells, strict=True)))
    if not rows:
        raise ValueError(f"{path} has a table header but no rows")
    return rows


def split_row(line: str) -> list[str]:
    return [cell.strip() for cell in line.strip().strip("|").split("|")]


def numeric(value: object) -> float:
    text = str(value).strip().replace("+", "").replace("~", "").replace(" min", "")
    return math.nan if not text else float(text)


def short_author(author: str) -> str:
    if "122B" in author:
        return "122B"
    if "35B" in author:
        return "35B"
    return author


def short_mode(mode: str) -> str:
    if mode == "Deepening":
        return "Deep"
    if mode == "Creation":
        return "Create"
    return mode


def rows_by(rows: list[dict[str, str]], column: str) -> dict[str, dict[str, str]]:
    return {row[column]: row for row in rows}


def panel_same_model_deepening(main_rows: list[dict[str, str]]) -> list[dict[str, Any]]:
    reference = rows_by([row for row in main_rows if row["Skill Author"] == "Reference"], "Condition")
    evolved = [
        row
        for row in main_rows
        if row["Mode"] == "Deepening"
        and row["Condition"] in {"+Error", "+Combined"}
        and row["Skill Author"] in {"Qwen3.5-122B-A10B", "Qwen3.5-35B-A3B"}
    ]
    human_122 = numeric(reference["Human-Written"]["122B Vrf"])
    human_35 = numeric(reference["Human-Written"]["35B Vrf"])

    panel = [{"x_label": "Human\n122B", "value": round(human_122, 2)}]
    for condition in ["+Error", "+Combined"]:
        delta = next(
            row["122B Vrf"]
            for row in evolved
            if row["Skill Author"] == "Qwen3.5-122B-A10B" and row["Condition"] == condition
        )
        panel.append({"x_label": f"{condition}\n122B", "value": round(human_122 + numeric(delta), 2)})
    panel.append({"x_label": "Human\n35B", "value": round(human_35, 2)})
    for condition in ["+Error", "+Combined"]:
        delta = next(
            row["35B Vrf"]
            for row in evolved
            if row["Skill Author"] == "Qwen3.5-35B-A3B" and row["Condition"] == condition
        )
        panel.append({"x_label": f"{condition}\n35B", "value": round(human_35 + numeric(delta), 2)})
    return panel


def panel_avg_improvement(main_rows: list[dict[str, str]]) -> list[dict[str, Any]]:
    rows = [row for row in main_rows if row["Skill Author"] != "Reference"]
    return [
        {
            "x_label": f"{short_author(row['Skill Author'])}\n{short_mode(row['Mode'])} {row['Condition']}",
            "value": round(numeric(row["Avg"]), 2),
        }
        for row in rows
    ]


def panel_parallel_vs_sequential(seq_rows: list[dict[str, str]]) -> list[dict[str, Any]]:
    return [
        {
            "x_label": row["Condition"],
            "122B Vrf": round(numeric(row["122B Vrf"]), 2),
            "35B Vrf": round(numeric(row["35B Vrf"]), 2),
            "Time": round(numeric(row["Time"]), 2),
        }
        for row in seq_rows
    ]


def panel_reasoningbank(rb_rows: list[dict[str, str]]) -> list[dict[str, Any]]:
    metrics = ["122B Vrf", "122B Soft", "122B Hard", "35B Vrf", "35B Soft", "35B Hard"]
    rows = rows_by(rb_rows, "Condition")
    return [
        {
            "metric": metric,
            "ReasoningBank": round(numeric(rows["ReasoningBank"][metric]), 2),
            "Trace2Skill skill": round(numeric(rows["Human-Written+Combined (ours)"][metric]), 2),
        }
        for metric in metrics
    ]


def load_result_records(ara_root: Path) -> list[dict[str, Any]]:
    records: list[dict[str, Any]] = []
    results_dir = ara_root / "results"
    for path in sorted(results_dir.glob("*.jsonl")):
        for line_number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), start=1):
            if not line.strip():
                continue
            record = json.loads(line)
            record["_source"] = f"{path.relative_to(ara_root).as_posix()}:{line_number}"
            records.append(record)
    return records


def build_provenance(ara_root: Path) -> dict[str, Any]:
    main_rows = parse_markdown_table(ara_root / "evidence/tables/table_main_spreadsheetbench.md")
    seq_rows = parse_markdown_table(ara_root / "evidence/tables/table_parallel_vs_sequential.md")
    rb_rows = parse_markdown_table(ara_root / "evidence/tables/table_reasoningbank.md")
    result_records = load_result_records(ara_root)
    plot_path = ara_root / PLOT_FILE
    if not plot_path.is_file():
        raise FileNotFoundError(f"missing target plot: {plot_path}")
    plot_bytes = plot_path.read_bytes()
    if not plot_bytes.startswith(b"\x89PNG\r\n\x1a\n"):
        raise ValueError(f"{plot_path} is not a PNG")

    overlay_records = [record for record in result_records if record.get("plot_binding") is not None]
    non_overlay_records = [record for record in result_records if record.get("plot_binding") is None]
    paper_denominator_records = [
        record for record in result_records if record.get("proof_classification") in PAPER_DENOMINATOR_CLASSIFICATIONS
    ]

    return {
        "schema_version": SCHEMA_VERSION,
        "plot": {
            "path": PLOT_FILE,
            "bytes": plot_path.stat().st_size,
            "sha256": sha256_file(plot_path),
            "kind": "paper-target-sheet",
        },
        "input_tables": [
            {
                "path": rel,
                "bytes": (ara_root / rel).stat().st_size,
                "sha256": sha256_file(ara_root / rel),
            }
            for rel in TARGET_TABLES
        ],
        "panels": {
            "same_model_deepening_vrf": panel_same_model_deepening(main_rows),
            "avg_improvement": panel_avg_improvement(main_rows),
            "parallel_vs_sequential": panel_parallel_vs_sequential(seq_rows),
            "reasoningbank": panel_reasoningbank(rb_rows),
        },
        "result_overlay_summary": {
            "records_total": len(result_records),
            "overlay_records": len(overlay_records),
            "non_overlay_records": len(non_overlay_records),
            "paper_denominator_records": len(paper_denominator_records),
            "sources": [record["_source"] for record in result_records],
            "overlay_sources": [record["_source"] for record in overlay_records],
            "non_overlay_sources": [record["_source"] for record in non_overlay_records],
        },
        "limits": [
            "Paper targets are not Leaven reproduction results.",
            "Records with plot_binding=null are intentionally not overlaid.",
            "Full paper reproduction requires paper-denominator result rows, not this plot manifest.",
        ],
    }


def check_plot_provenance(ara_root: Path) -> list[str]:
    errors: list[str] = []
    provenance_path = ara_root / PROVENANCE_FILE
    if not provenance_path.is_file():
        return [f"missing plot provenance: {provenance_path}"]
    try:
        expected = build_provenance(ara_root)
    except (FileNotFoundError, ValueError, json.JSONDecodeError) as exc:
        return [str(exc)]
    actual = json.loads(provenance_path.read_text(encoding="utf-8"))
    if actual != expected:
        errors.append(f"{provenance_path} is stale; regenerate with --write")
    summary = actual.get("result_overlay_summary", {})
    if summary.get("paper_denominator_records") != 0:
        errors.append("plot provenance must report zero paper_denominator_records until paper denominator is run")
    if summary.get("overlay_records") != 0:
        errors.append("plot provenance must report zero overlay_records until real overlay rows exist")
    if summary.get("non_overlay_records") != 1:
        errors.append("plot provenance must report one non_overlay_records entry for deterministic one-case")
    return errors


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "ara_dir",
        type=Path,
        default=Path("docs/ara/trace2skill_spreadsheetbench"),
        nargs="?",
    )
    parser.add_argument("--write", action="store_true", help="Write the expected provenance JSON before checking.")
    args = parser.parse_args()

    ara_root = args.ara_dir.resolve()
    provenance_path = ara_root / PROVENANCE_FILE
    if args.write:
        provenance_path.parent.mkdir(parents=True, exist_ok=True)
        provenance_path.write_text(json.dumps(build_provenance(ara_root), indent=2, sort_keys=True) + "\n")
        print(provenance_path)

    errors = check_plot_provenance(ara_root)
    if errors:
        for error in errors:
            print(f"ERROR: {error}", file=sys.stderr)
        return 1
    print(f"PASS: {args.ara_dir} plot provenance")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
