# Leaven Result Records

This directory is reserved for Leaven-produced result records that can be plotted
against the Trace2Skill paper targets.

No file in this directory is a paper target. No file in this directory should be
created from historical planning YAML, fixed fixture claims, mechanics tests, or
target-table transcription. A row belongs here only when a Leaven command
produced the named metric, the denominator is explicit, and the artifact paths
can be inspected.

## Current State

No Leaven overlay JSONL exists yet.

The deterministic one-case ACP worker result is recorded in
[`one_case_live.md`](one_case_live.md). It is intentionally not plotted against
paper target bars because its denominator is one local solved case, not a full
paper split or Qwen/vLLM aggregate.

The current acceptance and denominator audit is recorded in
[`denominator_status.md`](denominator_status.md).

Current public model/serving availability research for the approval packet is
recorded in [`model_availability.md`](model_availability.md).

The paper target plot can be regenerated without result records:

```bash
uv run --with matplotlib --with pandas python scripts/plot_trace2skill_ara.py docs/ara/trace2skill_spreadsheetbench
```

When real result files are present, the same command reads
`results/*.jsonl` and overlays Leaven points on the paper target sheet. The
target tables remain unchanged.

## Required Format

Use newline-delimited JSON. Each line must match
[`leaven_result_schema.md`](leaven_result_schema.md).

Each row represents one measured Leaven metric, not a whole experiment. If one
run reports `122B Vrf`, `122B Soft`, and runtime, write three rows with the same
`run_id` and different `metric_name` / `plot_binding`.

## Proof Classification

Allowed `proof_classification` values:

| Value | Meaning | Can close 1:1 reproduction? |
|-------|---------|-----------------------------|
| `mechanics-smoke` | Leaven plumbing works on deterministic or proxy data. | No |
| `deterministic-one-case` | The public seam solved one real SpreadsheetBench case with inspectable artifacts. | No |
| `paper-subset` | A real subset of the paper denominator ran with stated deviations. | No |
| `paper-denominator-candidate` | A run targets the paper denominator but is not yet seed/model aggregate complete. | No |
| `paper-denominator-reproduction` | The row belongs to the approved full denominator: paper split, seeds, model IDs, serving path, and artifact audit. | Yes, only with matching closeout evidence |

## Plot Binding

Every result row must declare where it can be overlaid:

```json
{
  "plot_binding": {
    "panel": "same_model_deepening_vrf",
    "x_label": "+Combined\n122B",
    "series": "Leaven",
    "axis": "left"
  }
}
```

Supported panels:

| Panel | `x_label` must match |
|-------|----------------------|
| `same_model_deepening_vrf` | One displayed x label in the same-model Vrf panel. |
| `avg_improvement` | One displayed x label in the average-improvement panel. |
| `parallel_vs_sequential` | One condition label in the parallel-vs-sequential panel. Use `axis: "right"` only for runtime minutes. |
| `reasoningbank` | One metric label in the ReasoningBank comparison panel. |

This explicit binding is intentional. The plotter must not guess which paper row
a Leaven metric is meant to compare against.
