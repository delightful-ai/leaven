# Trace2Skill Target Plots

This directory contains plots generated from ARA evidence tables.

The plots are **paper target sheets**, not Leaven reproduction results. They are
useful because they define the scoreboard Leaven must match or explain, but they
do not prove any live run, one-case proof, held-out split, seed aggregate, or
Qwen/vLLM parity.

Generate the current target sheet with:

```bash
uv run --with matplotlib --with pandas python scripts/plot_trace2skill_ara.py docs/ara/trace2skill_spreadsheetbench
```

Expected output:

```text
docs/ara/trace2skill_spreadsheetbench/plots/trace2skill_targets.png
```

The plot provenance manifest is checked with:

```bash
uv run --with pyyaml python scripts/check_trace2skill_plot_provenance.py docs/ara/trace2skill_spreadsheetbench
```

Regenerate the manifest after an intentional plot or evidence-table change with:

```bash
uv run --with pyyaml python scripts/check_trace2skill_plot_provenance.py docs/ara/trace2skill_spreadsheetbench --write
```

Current plot panels:

- same-model Deepening baseline versus evolved SpreadsheetBench-Verified Vrf;
- average improvement across paper slices;
- parallel consolidation versus sequential editing;
- distilled portable skill versus ReasoningBank retrieval memory.

Leaven overlays read separate result records from `results/*.jsonl` and display
a separate legend or marker. If no result JSONL files exist, the plotter renders
paper targets only. Do not edit paper evidence values to make a plot.

The current manifest records zero overlay records and one non-overlay
deterministic one-case record. That is intentional: the one-case row has
`plot_binding: null` and must not be drawn on full-paper target axes.
