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

This rejects missing artifact paths, absolute artifact paths, mechanics or
one-case rows with plot bindings, approval-gated rows missing runbook-required
prompt artifacts, and overlay rows whose metric unit, denominator, or paper
classification is not compatible with the target panel.

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
exist locally when the importer runs. Every row must carry
`extra.runbook_stage_id` naming a stage in `full_denominator_runbook.json`; that
stage's `allowed_label` must match the row's `proof_classification`. If that
runbook stage expects rendered prompts, fanout files, or prompt-render
manifests, result intake also requires matching prompt artifacts in
`artifact_paths`. The importer runs that same result-intake check before
writing its output, so rows with a wrong runbook stage or missing prompt
artifacts fail without producing a JSONL artifact. Result intake also checks
the `dataset_slice` against the runbook stage's generated
`expected_dataset_slice`, so subset rows cannot silently become 200-case
paper-denominator rows and held-out rows must use the paper `200..400` split.
It also checks the runbook stage's generated `expected_seed_policy`, so
approval-gated rows cannot drift to off-protocol seeds while still looking like
paper-denominator progress. Runtime fields are checked against generated
`expected_runtime_policy`, so rows must carry the paper worker count, ReAct turn
budget, and merge batch size where the originating stage requires them.

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
| `parallel_vs_sequential` | One condition label in the parallel-vs-sequential panel. Use `axis: "right"` only for runtime minutes. |
| `reasoningbank` | One metric label in the ReasoningBank comparison panel. |

This explicit binding is intentional. The plotter must not guess which paper row
a Leaven metric is meant to compare against.

Use `plot_binding: null` only for non-overlay result rows whose denominator
would be misleading on the paper-target panels.
