#!/usr/bin/env python3
"""Plot Trace2Skill paper targets from ARA evidence tables.

The generated figure is a target sheet. It is not Leaven reproduction evidence
unless separate Leaven result overlays are added later.
"""

from __future__ import annotations

import argparse
import importlib.util
import json
import math
import sys
from pathlib import Path
from typing import Any

import matplotlib.pyplot as plt
import pandas as pd

SCHEMA_VERSION = "leaven.trace2skill.result.v1"
ALLOWED_PROOF_CLASSIFICATIONS = {
    "mechanics-smoke",
    "deterministic-one-case",
    "model-one-case",
    "paper-subset",
    "evolving-split-run",
    "training-validation-candidate",
    "held-out-single-seed-candidate",
    "seed-aggregate-candidate",
    "paper-denominator-candidate",
    "paper-denominator-reproduction",
}
ALLOWED_METRIC_UNITS = {"percent", "delta_points", "minutes", "fraction"}
SUPPORTED_PANELS = {
    "same_model_deepening_vrf",
    "avg_improvement",
    "parallel_vs_sequential",
    "reasoningbank",
}


def parse_markdown_table(path: Path) -> pd.DataFrame:
    lines = path.read_text(encoding="utf-8").splitlines()
    table_lines = [line.strip() for line in lines if line.strip().startswith("|")]
    if len(table_lines) < 2:
        raise ValueError(f"{path} does not contain a Markdown table")

    header = split_row(table_lines[0])
    rows: list[list[str]] = []
    for line in table_lines[2:]:
        cells = split_row(line)
        if len(cells) == len(header):
            rows.append(cells)

    if not rows:
        raise ValueError(f"{path} has a table header but no rows")
    return pd.DataFrame(rows, columns=header)


def split_row(line: str) -> list[str]:
    return [cell.strip() for cell in line.strip().strip("|").split("|")]


def numeric(value: object) -> float:
    text = str(value).strip()
    text = text.replace("+", "")
    text = text.replace("~", "")
    text = text.replace(" min", "")
    if not text:
        return math.nan
    return float(text)


def require_columns(df: pd.DataFrame, columns: list[str], table_name: str) -> None:
    missing = [column for column in columns if column not in df.columns]
    if missing:
        raise ValueError(f"{table_name} missing columns: {', '.join(missing)}")


def default_result_paths(ara_dir: Path) -> list[Path]:
    results_dir = ara_dir / "results"
    if not results_dir.is_dir():
        return []
    return sorted(results_dir.glob("*.jsonl"))


def repo_root_for(ara_root: Path) -> Path:
    for candidate in (ara_root, *ara_root.parents):
        if (candidate / "scripts/check_trace2skill_result_intake.py").is_file():
            return candidate
    return Path.cwd()


def check_result_intake(repo_root: Path, ara_dir: Path, paths: list[Path]) -> None:
    if not paths:
        return
    checker_path = repo_root / "scripts/check_trace2skill_result_intake.py"
    spec = importlib.util.spec_from_file_location("check_trace2skill_result_intake", checker_path)
    if spec is None or spec.loader is None:
        raise ValueError(f"cannot import result-intake checker: {checker_path}")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)

    runbook = json.loads((ara_dir / "results/full_denominator_runbook.json").read_text(encoding="utf-8"))
    runbook_stages = {
        stage["id"]: stage
        for stage in runbook.get("stages", [])
        if isinstance(stage, dict) and isinstance(stage.get("id"), str)
    }
    errors: list[str] = []
    for path in paths:
        resolved = path.resolve()
        try:
            rows = module.load_jsonl(resolved)
        except (json.JSONDecodeError, OSError, ValueError) as exc:
            errors.append(f"{path} is not valid result JSONL: {exc}")
            continue
        for line_number, record in rows:
            try:
                resolved.relative_to(repo_root)
            except ValueError:
                errors.append(f"{path}:{line_number} result files must live under the repo root")
                continue
            approval_blockers = (
                module.approval_packet_errors(ara_dir)
                if (
                    record.get("proof_classification") in module.APPROVAL_REQUIRED_PROOF_CLASSIFICATIONS
                    and record.get("plot_binding") is not None
                )
                else None
            )
            extra = record.get("extra")
            source_approval_blockers = (
                module.approval_packet_errors(ara_dir)
                if isinstance(extra, dict) and extra.get("source_result_paths")
                else None
            )
            module.check_record(
                repo_root,
                resolved,
                line_number,
                record,
                runbook_stages,
                errors,
                approval_blockers=approval_blockers,
                source_approval_blockers=source_approval_blockers,
            )
    if errors:
        detail = "; ".join(errors)
        raise ValueError(f"result overlays failed result intake: {detail}")


def load_result_records(paths: list[Path]) -> list[dict[str, Any]]:
    records: list[dict[str, Any]] = []
    for path in paths:
        for line_number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), start=1):
            if not line.strip():
                continue
            try:
                record = json.loads(line)
            except json.JSONDecodeError as exc:
                raise ValueError(f"{path}:{line_number} is not valid JSON: {exc}") from exc
            validate_result_record(record, path, line_number)
            record["_source"] = f"{path}:{line_number}"
            records.append(record)
    return records


def validate_result_record(record: Any, path: Path, line_number: int) -> None:
    prefix = f"{path}:{line_number}"
    if not isinstance(record, dict):
        raise ValueError(f"{prefix} must be a JSON object")

    required = [
        "schema_version",
        "run_id",
        "created_at",
        "proof_classification",
        "dataset_slice",
        "model_id",
        "seed",
        "skill_source",
        "metric_name",
        "metric_value",
        "metric_unit",
        "plot_binding",
        "cost",
        "runtime",
        "source_command",
        "artifact_paths",
        "notes",
    ]
    missing = [field for field in required if field not in record]
    if missing:
        raise ValueError(f"{prefix} missing fields: {', '.join(missing)}")
    if record["schema_version"] != SCHEMA_VERSION:
        raise ValueError(f"{prefix} schema_version must be {SCHEMA_VERSION}")
    if record["proof_classification"] not in ALLOWED_PROOF_CLASSIFICATIONS:
        raise ValueError(f"{prefix} has invalid proof_classification")
    if record["metric_unit"] not in ALLOWED_METRIC_UNITS:
        raise ValueError(f"{prefix} has invalid metric_unit")
    if not isinstance(record["metric_value"], (int, float)) or isinstance(record["metric_value"], bool):
        raise ValueError(f"{prefix} metric_value must be numeric")

    for field in ("run_id", "created_at", "model_id", "metric_name", "source_command", "notes"):
        if not isinstance(record[field], str):
            raise ValueError(f"{prefix} {field} must be a string")
    for field in ("run_id", "created_at", "model_id", "metric_name", "source_command"):
        if not record[field].strip():
            raise ValueError(f"{prefix} {field} must be non-empty")

    dataset_slice = record["dataset_slice"]
    if not isinstance(dataset_slice, dict):
        raise ValueError(f"{prefix} dataset_slice must be an object")
    for field in ("name", "split", "case_count", "denominator"):
        if field not in dataset_slice:
            raise ValueError(f"{prefix} dataset_slice missing {field}")
    if not isinstance(dataset_slice["case_count"], int) or dataset_slice["case_count"] < 1:
        raise ValueError(f"{prefix} dataset_slice.case_count must be a positive integer")
    for field in ("name", "split", "denominator"):
        if not isinstance(dataset_slice[field], str) or not dataset_slice[field].strip():
            raise ValueError(f"{prefix} dataset_slice.{field} must be a non-empty string")

    skill_source = record["skill_source"]
    if not isinstance(skill_source, dict) or not isinstance(skill_source.get("kind"), str) or not skill_source["kind"]:
        raise ValueError(f"{prefix} skill_source.kind must be a non-empty string")
    for field in ("cost", "runtime"):
        if not isinstance(record[field], dict):
            raise ValueError(f"{prefix} {field} must be an object")

    binding = record["plot_binding"]
    if binding is not None and not isinstance(binding, dict):
        raise ValueError(f"{prefix} plot_binding must be an object or null")
    if isinstance(binding, dict):
        for field in ("panel", "x_label", "series", "axis"):
            if not isinstance(binding.get(field), str) or not binding[field]:
                raise ValueError(f"{prefix} plot_binding.{field} must be a non-empty string")
        if binding["panel"] not in SUPPORTED_PANELS:
            raise ValueError(f"{prefix} plot_binding.panel is not supported")
        if binding["axis"] not in {"left", "right"}:
            raise ValueError(f"{prefix} plot_binding.axis must be left or right")

    artifacts = record["artifact_paths"]
    if not isinstance(artifacts, list) or not artifacts:
        raise ValueError(f"{prefix} artifact_paths must be a non-empty array")
    if any(not isinstance(path_entry, str) or not path_entry for path_entry in artifacts):
        raise ValueError(f"{prefix} artifact_paths entries must be non-empty strings")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("ara_dir", type=Path)
    parser.add_argument("--output", type=Path)
    parser.add_argument(
        "--results",
        type=Path,
        action="append",
        help="Result JSONL file to overlay. Defaults to ara_dir/results/*.jsonl.",
    )
    parser.add_argument(
        "--no-result-overlays",
        action="store_true",
        help="Render paper targets only, even if ara_dir/results/*.jsonl exists.",
    )
    args = parser.parse_args()

    ara_dir = args.ara_dir
    repo_root = repo_root_for(ara_dir.resolve()).resolve()
    tables_dir = ara_dir / "evidence/tables"
    output = args.output or ara_dir / "plots/trace2skill_targets.png"
    output.parent.mkdir(parents=True, exist_ok=True)
    result_paths = [] if args.no_result_overlays else (args.results or default_result_paths(ara_dir))
    check_result_intake(repo_root, ara_dir.resolve(), result_paths)
    result_records = load_result_records(result_paths)

    main_table = parse_markdown_table(tables_dir / "table_main_spreadsheetbench.md")
    seq_table = parse_markdown_table(tables_dir / "table_parallel_vs_sequential.md")
    rb_table = parse_markdown_table(tables_dir / "table_reasoningbank.md")

    require_columns(
        main_table,
        ["Skill Author", "Mode", "Condition", "122B Vrf", "35B Vrf", "Avg"],
        "table_main_spreadsheetbench.md",
    )
    require_columns(
        seq_table,
        ["Condition", "122B Vrf", "122B Soft", "122B Hard", "35B Vrf", "35B Soft", "35B Hard", "Time"],
        "table_parallel_vs_sequential.md",
    )
    require_columns(
        rb_table,
        ["Condition", "122B Vrf", "122B Soft", "122B Hard", "35B Vrf", "35B Soft", "35B Hard"],
        "table_reasoningbank.md",
    )

    fig, axes = plt.subplots(2, 2, figsize=(15, 10), constrained_layout=True)
    fig.suptitle("Trace2Skill paper targets from ARA evidence", fontsize=18, fontweight="bold")

    plotted_results: set[int] = set()
    plot_baseline_vs_evolved(axes[0, 0], main_table, result_records, plotted_results)
    plot_avg_improvement(axes[0, 1], main_table, result_records, plotted_results)
    plot_parallel_vs_sequential(axes[1, 0], seq_table, result_records, plotted_results)
    plot_reasoningbank(axes[1, 1], rb_table, result_records, plotted_results)
    unplotted = [record.get("_source", "<unknown>") for index, record in enumerate(result_records) if index not in plotted_results]
    if unplotted:
        raise ValueError(f"result records were valid but not plotted: {', '.join(unplotted)}")

    fig.text(
        0.01,
        0.01,
        "Source: docs/ara/trace2skill_spreadsheetbench/evidence/tables/*.md. "
        "This is a paper target sheet, not a Leaven reproduction result.",
        fontsize=9,
    )
    fig.savefig(output, dpi=180)
    print(output)
    return 0


def plot_baseline_vs_evolved(
    ax: plt.Axes,
    main_table: pd.DataFrame,
    result_records: list[dict[str, Any]],
    plotted_results: set[int],
) -> None:
    reference = main_table[main_table["Skill Author"] == "Reference"].set_index("Condition")
    evolved = main_table[
        (main_table["Mode"] == "Deepening")
        & (main_table["Condition"].isin(["+Error", "+Combined"]))
        & (main_table["Skill Author"].isin(["Qwen3.5-122B-A10B", "Qwen3.5-35B-A3B"]))
    ]

    human_122 = numeric(reference.loc["Human-Written", "122B Vrf"])
    human_35 = numeric(reference.loc["Human-Written", "35B Vrf"])

    rows = [
        ("Human\n122B", human_122),
    ]
    for condition in ["+Error", "+Combined"]:
        delta = evolved[
            (evolved["Skill Author"] == "Qwen3.5-122B-A10B") & (evolved["Condition"] == condition)
        ]["122B Vrf"].iloc[0]
        rows.append((f"{condition}\n122B", human_122 + numeric(delta)))

    rows.append(("Human\n35B", human_35))
    for condition in ["+Error", "+Combined"]:
        delta = evolved[
            (evolved["Skill Author"] == "Qwen3.5-35B-A3B") & (evolved["Condition"] == condition)
        ]["35B Vrf"].iloc[0]
        rows.append((f"{condition}\n35B", human_35 + numeric(delta)))

    labels, values = zip(*rows)
    colors = ["#6b7280", "#00a884", "#0096c7", "#6b7280", "#00a884", "#0096c7"]
    ax.bar(labels, values, color=colors)
    ax.set_title("SpreadsheetBench-Verified Vrf: same-model deepening")
    ax.set_ylabel("paper target score (%)")
    ax.set_ylim(0, 80)
    for i, value in enumerate(values):
        ax.text(i, value + 1, f"{value:.2f}", ha="center", fontsize=8)
    overlay_points(ax, result_records, plotted_results, "same_model_deepening_vrf", dict(enumerate(labels)))


def plot_avg_improvement(
    ax: plt.Axes,
    main_table: pd.DataFrame,
    result_records: list[dict[str, Any]],
    plotted_results: set[int],
) -> None:
    rows = main_table[main_table["Skill Author"] != "Reference"].copy()
    rows["AvgValue"] = rows["Avg"].map(numeric)
    labels = [
        f"{short_author(row['Skill Author'])}\n{short_mode(row['Mode'])} {row['Condition']}"
        for _, row in rows.iterrows()
    ]
    values = rows["AvgValue"].tolist()
    colors = ["#00a884" if value >= 0 else "#e11d48" for value in values]
    ax.bar(range(len(values)), values, color=colors)
    ax.axhline(0, color="black", linewidth=0.8)
    ax.set_title("Average improvement across paper slices")
    ax.set_ylabel("paper target delta points")
    ax.set_xticks(range(len(values)), labels, rotation=45, ha="right", fontsize=7)
    overlay_points(ax, result_records, plotted_results, "avg_improvement", dict(enumerate(labels)))


def plot_parallel_vs_sequential(
    ax: plt.Axes,
    seq_table: pd.DataFrame,
    result_records: list[dict[str, Any]],
    plotted_results: set[int],
) -> None:
    labels = seq_table["Condition"].tolist()
    x = list(range(len(labels)))
    v122 = [numeric(value) for value in seq_table["122B Vrf"]]
    v35 = [numeric(value) for value in seq_table["35B Vrf"]]
    times = [numeric(value) for value in seq_table["Time"]]

    ax.plot(x, v122, marker="o", label="122B Vrf", color="#2563eb")
    ax.plot(x, v35, marker="o", label="35B Vrf", color="#f97316")
    ax.set_xticks(x, labels)
    ax.set_ylabel("paper target Vrf (%)")
    ax.set_title("Parallel consolidation vs sequential editing")
    ax.legend(loc="upper left")

    ax2 = ax.twinx()
    ax2.bar([i + 0.18 for i in x], times, width=0.25, alpha=0.25, color="#111827")
    ax2.set_ylabel("paper runtime minutes")
    for i, value in enumerate(times):
        ax2.text(i + 0.18, value + 1, f"{value:.0f}m", ha="center", fontsize=8)
    x_positions = dict(enumerate(labels))
    overlay_points(ax, result_records, plotted_results, "parallel_vs_sequential", x_positions, axis="left")
    overlay_points(ax2, result_records, plotted_results, "parallel_vs_sequential", x_positions, axis="right")


def plot_reasoningbank(
    ax: plt.Axes,
    rb_table: pd.DataFrame,
    result_records: list[dict[str, Any]],
    plotted_results: set[int],
) -> None:
    metrics = ["122B Vrf", "122B Soft", "122B Hard", "35B Vrf", "35B Soft", "35B Hard"]
    reasoning = rb_table[rb_table["Condition"] == "ReasoningBank"].iloc[0]
    ours = rb_table[rb_table["Condition"] == "Human-Written+Combined (ours)"].iloc[0]
    rb_values = [numeric(reasoning[metric]) for metric in metrics]
    ours_values = [numeric(ours[metric]) for metric in metrics]

    x = list(range(len(metrics)))
    width = 0.35
    ax.bar([i - width / 2 for i in x], rb_values, width=width, label="ReasoningBank", color="#9ca3af")
    ax.bar([i + width / 2 for i in x], ours_values, width=width, label="Trace2Skill skill", color="#7c3aed")
    ax.set_xticks(x, metrics, rotation=35, ha="right")
    ax.set_ylabel("paper target score (%)")
    ax.set_title("Portable skill vs retrieval memory")
    ax.legend()
    overlay_points(ax, result_records, plotted_results, "reasoningbank", dict(enumerate(metrics)))


def overlay_points(
    ax: plt.Axes,
    result_records: list[dict[str, Any]],
    plotted_results: set[int],
    panel: str,
    indexed_labels: dict[int, str],
    axis: str = "left",
) -> None:
    label_positions = {label: index for index, label in indexed_labels.items()}
    for index, record in enumerate(result_records):
        binding = record["plot_binding"]
        if binding is None:
            plotted_results.add(index)
            continue
        if binding["panel"] != panel or binding["axis"] != axis:
            continue
        x_label = binding["x_label"]
        if x_label not in label_positions:
            source = record.get("_source", "<unknown>")
            labels = ", ".join(repr(label) for label in label_positions)
            raise ValueError(f"{source} plot_binding.x_label {x_label!r} does not match panel labels: {labels}")
        x = label_positions[x_label]
        y = float(record["metric_value"])
        ax.scatter(
            [x],
            [y],
            marker="D",
            s=74,
            facecolors="white",
            edgecolors="#111827",
            linewidths=1.5,
            label=binding["series"],
            zorder=5,
        )
        ax.text(x, y, short_proof(record["proof_classification"]), ha="left", va="bottom", fontsize=7)
        plotted_results.add(index)
    dedupe_legend(ax)


def dedupe_legend(ax: plt.Axes) -> None:
    handles, labels = ax.get_legend_handles_labels()
    if not handles:
        return
    deduped: dict[str, Any] = {}
    for handle, label in zip(handles, labels, strict=True):
        if label not in deduped:
            deduped[label] = handle
    ax.legend(deduped.values(), deduped.keys(), loc="best")


def short_proof(proof: str) -> str:
    return {
        "mechanics-smoke": "mech",
        "deterministic-one-case": "1case",
        "paper-subset": "subset",
        "paper-denominator-candidate": "candidate",
        "paper-denominator-reproduction": "1:1",
    }.get(proof, proof)


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


if __name__ == "__main__":
    raise SystemExit(main())
