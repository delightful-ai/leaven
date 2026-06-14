#!/usr/bin/env python3
"""Plot Trace2Skill paper targets from ARA evidence tables.

The generated figure is a target sheet. It is not Leaven reproduction evidence
unless separate Leaven result overlays are added later.
"""

from __future__ import annotations

import argparse
import math
from pathlib import Path

import matplotlib.pyplot as plt
import pandas as pd


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


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("ara_dir", type=Path)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()

    ara_dir = args.ara_dir
    tables_dir = ara_dir / "evidence/tables"
    output = args.output or ara_dir / "plots/trace2skill_targets.png"
    output.parent.mkdir(parents=True, exist_ok=True)

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

    plot_baseline_vs_evolved(axes[0, 0], main_table)
    plot_avg_improvement(axes[0, 1], main_table)
    plot_parallel_vs_sequential(axes[1, 0], seq_table)
    plot_reasoningbank(axes[1, 1], rb_table)

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


def plot_baseline_vs_evolved(ax: plt.Axes, main_table: pd.DataFrame) -> None:
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


def plot_avg_improvement(ax: plt.Axes, main_table: pd.DataFrame) -> None:
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


def plot_parallel_vs_sequential(ax: plt.Axes, seq_table: pd.DataFrame) -> None:
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


def plot_reasoningbank(ax: plt.Axes, rb_table: pd.DataFrame) -> None:
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
