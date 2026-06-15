# Leaven Result Records

This directory is reserved for Leaven-produced result records. Some can be
plotted against Trace2Skill paper targets; others are non-overlay rows when
their denominator would make a paper-target overlay misleading.

No file in this directory is a paper target. No file in this directory should be
created from historical planning YAML, fixed fixture claims, mechanics tests, or
target-table transcription. A row belongs here only when a Leaven command
produced the named metric, the denominator is explicit, and the artifact paths
can be inspected.

## Current State

One non-overlay Leaven JSONL exists for the deterministic one-case proof. No
paper-denominator overlay JSONL exists yet.

The deterministic one-case ACP worker result is recorded in
[`one_case_live.md`](one_case_live.md) and
[`deterministic_one_case.jsonl`](deterministic_one_case.jsonl). It is
intentionally not plotted against paper target bars because its denominator is
one local solved case, not a full paper split or Qwen/vLLM aggregate.

The current acceptance and denominator audit is recorded in
[`denominator_status.md`](denominator_status.md).

Current public model/serving availability research for the approval packet is
recorded in [`model_availability.md`](model_availability.md).

The local 400-row SpreadsheetBench-Verified dataset manifest is recorded in
[`dataset_manifest.json`](dataset_manifest.json) and can be rebuilt with:

```bash
uv run python scripts/build_trace2skill_dataset_manifest.py
```

The deterministic one-case result record can be rebuilt with:

```bash
uv run python scripts/build_trace2skill_one_case_result.py
```

Its `plot_binding` is `null` so it is not drawn on paper-denominator target
plots.

All result JSONL files are checked with:

```bash
uv run --with pyyaml python scripts/check_trace2skill_result_intake.py docs/ara/trace2skill_spreadsheetbench
```

This first enforces the base Leaven result envelope from
[`leaven_result_schema.md`](leaven_result_schema.md): schema id, run id,
created timestamp, proof classification, dataset slice, model id, seed shape,
serving backend, skill source, metric fields, cost/runtime objects, source
command, artifacts, extra object, and notes. It then rejects missing artifact
paths, absolute artifact paths, mechanics or one-case rows with plot bindings,
approval-gated rows missing runbook-required prompt artifacts, full paper
reproduction rows whose model or serving backend does not match the paper
protocol, approval-gated paper-protocol rows whose model or serving backend
does not match the paper protocol, and overlay rows whose metric unit,
denominator, or paper classification is not compatible with the target panel.
Overlay rows must also
use a `plot_binding.x_label` present in the committed target-plot provenance
for that panel, so closeout cannot count a row the plotter would later reject.
When the target label names a model family such as `122B` or `35B`, known Qwen
paper model ids must match that family. For the `parallel_vs_sequential` score
panel, the x-label is only the editing condition; known Qwen paper model rows
on the left axis must name the matching `122B` or `35B` family in
`plot_binding.series`.

Official SpreadsheetBench evaluator output can be converted into Leaven result
rows only after a real Leaven run has produced an inspectable
`eval_official_results.json`:

```bash
uv run --with pyyaml python scripts/import_trace2skill_eval_results.py \
  --eval-results path/to/eval_official_results.json \
  --output docs/ara/trace2skill_spreadsheetbench/results/<run_id>.jsonl \
  --ara-dir docs/ara/trace2skill_spreadsheetbench \
  --run-id <run_id> \
  --created-at <timestamp> \
  --proof-classification paper-subset \
  --runbook-stage-id G2 \
  --split <split> \
  --case-range <start..end> \
  --case-count <n> \
  --denominator <explicit-denominator> \
  --model-id <model> \
  --serving-backend <backend> \
  --seed <seed> \
  --skill-kind trace2skill-evolved \
  --skill-path path/to/SKILL.md \
  --artifact-path path/to/rendered_prompts/<case_id>/agent_prompt.md \
  --artifact-path path/to/prompt_render_manifest.json \
  --approval-artifact-path docs/ara/trace2skill_spreadsheetbench/results/full_run_plan.md \
  --source-command '<exact command>'
```

The importer writes `plot_binding: null` by default. Paper-target overlays
require explicit `--plot-binding-json` entries. Every approval-gated proof
classification requires at least one `--approval-artifact-path`; this includes
`model-one-case`, `paper-subset`, `evolving-split-run`,
`training-validation-candidate`, `held-out-single-seed-candidate`,
`seed-aggregate-candidate`, `paper-denominator-candidate`, and
`paper-denominator-reproduction`. `paper-denominator-reproduction` also requires
`--allow-paper-denominator-reproduction`. Every `--eval-results`,
`--skill-path`, `--artifact-path`, and `--approval-artifact-path` value must
exist locally when the importer runs. The official evaluator payload must also
carry per-case `results[*].id` values matching the declared
`--case-range` in upstream SpreadsheetBench order; summary totals alone are not
enough to import a range-labeled result. When `--skill-path` is present, the
importer also writes that same path into `artifact_paths`; result intake rejects
file-backed `skill_source.path` metadata that is missing from the artifact
audit. Every row must carry
`extra.runbook_stage_id` naming a stage in `full_denominator_runbook.json`; that
stage's `allowed_label` must match the row's `proof_classification`. If that
runbook stage expects rendered prompts, fanout files, or prompt-render
manifests, result intake also requires matching prompt artifacts in
`artifact_paths`. If the importer output path is a top-level ARA
`results/*.jsonl` file, approval-gated rows also require the approval packet in
`results/full_run_plan.md` to pass the normal runnable preflight; merely naming
the blocked plan as an approval artifact is not permission. For model-backed run
stages, result intake also requires
file-shaped runbook artifacts such as prompt manifests, official evaluator
outputs, parsed analysis JSON, fanout JSONL, merge manifests, change logs, or
selection notes; directory placeholders and the row's own `leaven_results.jsonl`
are not used as artifact-path requirements. The importer runs that same
result-intake check before writing its output, so rows with a wrong runbook
stage or missing runbook-required file artifacts fail without producing a JSONL
artifact. Result intake also checks
the `dataset_slice` against the runbook stage's generated
`expected_dataset_slice`, so one-case rows must carry the generated
`extra.case_id` such as `13-1`, every stage with a generated split must use
that exact `dataset_slice.split`, subset rows cannot silently become 200-case
paper-denominator rows, held-out rows must use the paper `200..400` split, and
aggregate/full-paper rows must keep generated case ranges and counts.
All approval-gated paper-protocol rows (`model-one-case`, `paper-subset`,
`evolving-split-run`, `training-validation-candidate`,
`held-out-single-seed-candidate`, `seed-aggregate-candidate`,
`paper-denominator-candidate`, and `paper-denominator-reproduction`) must use a
paper model id (`Qwen3.5-122B-A10B` or `Qwen3.5-35B-A3B`) and `vLLM`, so
fixture-model rows cannot masquerade as model-backed one-case, subset,
evolving, validation, held-out, aggregate, or full-paper evidence.
It also checks the runbook stage's generated `expected_seed_policy`, so
approval-gated rows cannot drift to off-protocol seeds while still looking like
paper-denominator progress. Runtime fields are checked against generated
`expected_runtime_policy`, so rows must carry the paper worker count, ReAct turn
budget, and merge batch size where the originating stage requires them. Command
evidence is checked against generated `expected_command_policy`, so rows must
carry `extra.command_policy` and a `source_command` containing the upstream
command fragments required by the originating stage. Range-bearing rows must
also include concrete `--start_idx` and `--end_idx` fragments matching their
declared `dataset_slice.case_range`; a row cannot claim held-out range
`200..202` while its recorded command names `0..2`. Command-backed
approval-gated rows must also include a concrete `--model <model_id>` flag
matching the same paper `model_id`, and must not include another paper model in
a different `--model` flag. When the row carries `seed`, `runtime.workers`, or
`runtime.max_turns`, the command must include matching `--seeds`, `--workers`,
and `--max_turns` flags. Metadata cannot claim Qwen paper identity or paper run
settings when the recorded command names another model, seed, worker count, turn
budget, no concrete flag, or only mentions the value outside the executed flag.
Skill-evolution rows must also include concrete `--max_workers`,
`--max-workers`, and `--merge-batch-size` values in the command that match the
runtime worker count and `extra.merge_batch_size`; a row cannot rely on
metadata or on upstream defaults for the paper's analyst fanout, evolution
fanout, or batch size `32`.
The `analysis/run_error_analysis.py` command segment must also carry its own
matching `--model`, `--workers`, and `--max_turns` flags; a correct baseline
SpreadsheetBench command cannot stand in for the error-analysis invocation.
Official evaluator-derived rows must keep `extra.source_metric` tied to the
metric actually counted by the
evaluator. Non-overlay imports use canonical `official_*` metric names, and
overlays may bind raw official metrics only to compatible score panels; raw
official evaluator metrics cannot be relabeled as derived average-improvement
or runtime overlays. Aggregate evidence is
checked against generated `expected_aggregate_policy`, so seed aggregates must
cite inspectable held-out single-seed result JSONL rows that themselves pass
result intake for seeds `41`, `42`, and `43`, with the aggregate `metric_value`
equal to the mean of the cited seed metric values; every cited seed-aggregate
source row must be a G4 held-out single-seed candidate. Full-paper rows must
cite training-validation or seed-aggregate source rows that also pass result
intake and together cover the generated required split ranges such as `0..200`
and `200..400`, with the full-paper `metric_value` equal to the
case-count-weighted mean of those cited split metrics. Every cited full-paper
source row must have a full-paper source classification named by the generated
aggregate policy; a held-out single-seed row can support a seed aggregate, but
it cannot be cited directly as a full-paper predecessor. Aggregate and
full-paper source rows must also match the parent row's `model_id`,
`serving_backend`, `metric_name`, and `metric_unit`, so a seed aggregate or
full-paper row cannot silently combine incompatible model or metric conditions.
When a cited source row lives in top-level ARA
`results/*.jsonl`, approval-gated source rows must also pass the runnable
approval-packet preflight before they can count as aggregate or full-paper
support.

The full-denominator approval packet can be checked with:

```bash
uv run --with pyyaml python scripts/check_trace2skill_approval_packet.py docs/ara/trace2skill_spreadsheetbench
```

Until the packet is approved, the expected proof is the blocked preflight:

```bash
uv run --with pyyaml python scripts/check_trace2skill_approval_packet.py docs/ara/trace2skill_spreadsheetbench --expect-blocked
```

The closeout audit can be regenerated with:

```bash
uv run --with pyyaml python scripts/audit_trace2skill_closeout.py docs/ara/trace2skill_spreadsheetbench
```

It writes [`closeout_audit.md`](closeout_audit.md) and
[`closeout_audit.json`](closeout_audit.json). While the full paper denominator
is missing, the audit must keep `overall_complete` false.

The full-denominator runbook can be regenerated with:

```bash
uv run --with pyyaml python scripts/build_trace2skill_runbook.py docs/ara/trace2skill_spreadsheetbench
```

It writes [`full_denominator_runbook.md`](full_denominator_runbook.md) and
[`full_denominator_runbook.json`](full_denominator_runbook.json). It is a staged
execution map, not permission to launch model work.

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
| `model-one-case` | A model-backed run solved one real SpreadsheetBench case with inspectable artifacts. | No |
| `paper-subset` | A real subset of the paper denominator ran with stated deviations. | No |
| `evolving-split-run` | Rows `0..200` were used for trajectory collection and skill evolution only. | No |
| `training-validation-candidate` | Rows `0..200` were used to select or validate an evolved skill before held-out evaluation. | No |
| `held-out-single-seed-candidate` | Rows `200..400` were evaluated for one approved seed/model condition. | No |
| `seed-aggregate-candidate` | Seeds `41/42/43` were aggregated for one approved model/condition. | No |
| `paper-denominator-candidate` | A run targets the paper denominator but is not yet seed/model aggregate complete. | No |
| `paper-denominator-reproduction` | The row belongs to the approved full denominator: paper split, seeds, model IDs, serving path, and artifact audit. | Yes, only with matching closeout evidence |

## Plot Binding

Every overlay result row must declare where it can be overlaid:

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
| `parallel_vs_sequential` | One condition label in the parallel-vs-sequential panel. Use `axis: "right"` only for runtime minutes. Left-axis score overlays for known Qwen paper models must name the matching `122B` or `35B` family in `plot_binding.series`. |
| `reasoningbank` | One metric label in the ReasoningBank comparison panel. |

This explicit binding is intentional. The plotter must not guess which paper row
a Leaven metric is meant to compare against. Result intake also checks the
binding against the committed target-plot provenance before plots or closeout
can consume the row, and rejects known paper model ids bound to the wrong
model-family label or parallel-panel series family.

Use `plot_binding: null` only for non-overlay result rows whose denominator
would be misleading on the paper-target panels.
