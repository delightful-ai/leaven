# Trace2Skill Leaven Result JSONL Schema

Schema id: `leaven.trace2skill.result.v1`

This is the minimal result envelope for Leaven-produced measurements. Rows may
either bind to a paper-target plot panel or remain non-overlay records when the
denominator would make a paper-target overlay misleading. It is deliberately
stricter than the paper tables: paper targets say what to match; result rows say
what Leaven actually ran.

## Record Shape

```json
{
  "schema_version": "leaven.trace2skill.result.v1",
  "run_id": "trace2skill-2026-06-14T00-00-00Z-seed41",
  "created_at": "2026-06-14T00:00:00Z",
  "proof_classification": "paper-subset",
  "dataset_slice": {
    "name": "SpreadsheetBench",
    "split": "test",
    "case_range": "200..400",
    "case_count": 200,
    "denominator": "paper-held-out-spreadsheetbench-200-400"
  },
  "model_id": "Qwen3.5-122B-A10B",
  "serving_backend": "vLLM",
  "seed": 41,
  "skill_source": {
    "kind": "trace2skill-evolved",
    "path": "artifacts/trace2skill/run-.../skill/SKILL.md"
  },
  "metric_name": "122B Vrf",
  "metric_value": 68.88,
  "metric_unit": "percent",
  "plot_binding": {
    "panel": "same_model_deepening_vrf",
    "x_label": "+Combined\n122B",
    "series": "Leaven",
    "axis": "left"
  },
  "cost": {
    "usd": null,
    "prompt_tokens": null,
    "completion_tokens": null
  },
  "runtime": {
    "seconds": null,
    "workers": 128,
    "max_turns": 100
  },
  "source_command": "LEAVEN_TRACE2SKILL_LIVE=1 ...",
  "artifact_paths": [
    "artifacts/trace2skill/run-.../manifest.json",
    "artifacts/trace2skill/run-.../rendered_prompts/52807/agent_prompt.md",
    "artifacts/trace2skill/run-.../prompt_render_manifest.json",
    "artifacts/trace2skill/run-.../score_report.json",
    "artifacts/trace2skill/run-.../trajectory.jsonl",
    "docs/ara/trace2skill_spreadsheetbench/results/full_run_plan.md"
  ],
  "extra": {
    "runbook_stage_id": "G4",
    "command_policy": "upstream-eval",
    "approval_artifact_paths": [
      "docs/ara/trace2skill_spreadsheetbench/results/full_run_plan.md"
    ],
    "source_result_paths": []
  },
  "notes": "Deviation notes or empty string."
}
```

## Required Fields

| Field | Type | Rule |
|-------|------|------|
| `schema_version` | string | Must equal `leaven.trace2skill.result.v1`. |
| `run_id` | string | Stable id shared by rows from one run. |
| `created_at` | string | ISO-like timestamp for provenance. |
| `proof_classification` | string | Must be one of the values listed in `README.md`. |
| `dataset_slice` | object | Must include `name`, `split`, `case_count`, and `denominator`. |
| `model_id` | string | The model that produced the metric. |
| `serving_backend` | string | The serving backend that produced the metric; full paper-denominator reproduction rows must use `vLLM`. |
| `seed` | number, string, or null | Null only when the run truly had no seed. |
| `skill_source` | object | Must include `kind`; include `path` when the skill is file-backed. |
| `metric_name` | string | Human-readable metric label, e.g. `122B Vrf`. |
| `metric_value` | number | The plotted numeric value. |
| `metric_unit` | string | `percent`, `delta_points`, `minutes`, or `fraction`. |
| `plot_binding` | object or null | Object rows must include `panel`, `x_label`, `series`, and `axis`; null rows are valid non-overlay result records. |
| `cost` | object | Include available spend/token fields; use null for unknown fields. |
| `runtime` | object | Include `seconds`; include `workers` and `max_turns` when the runbook stage requires them. |
| `source_command` | string | Exact command or harness invocation that produced the metric. |
| `artifact_paths` | array | Non-empty list of inspectable artifacts. |
| `extra.runbook_stage_id` | string | Must name a stage in `full_denominator_runbook.json`; that stage's `allowed_label` must match `proof_classification`. |
| `extra.command_policy` | string | Required when the named runbook stage has `expected_command_policy`; must match that policy's `kind`. |
| `extra.source_metric` | string | Required for rows imported from official SpreadsheetBench evaluator summaries; must name the official source metric behind `metric_name` and any `plot_binding`. |
| `extra.source_result_paths` | array | Required when the named runbook stage has `expected_aggregate_policy`; each entry must be an inspectable result JSONL, must also appear in `artifact_paths`, and predecessor rows must pass result intake. |
| `extra.approval_artifact_paths` | array | Required for every approval-gated class: `model-one-case`, `paper-subset`, `evolving-split-run`, `training-validation-candidate`, `held-out-single-seed-candidate`, `seed-aggregate-candidate`, `paper-denominator-candidate`, and `paper-denominator-reproduction`. Each entry must also appear in `artifact_paths`. |
| `notes` | string | Empty string is allowed; missing is not. |

## Refusal Rules

- Do not write paper target rows as Leaven result rows.
- Do not write rows from historical YAML claims unless the underlying artifacts
  and command can be inspected now.
- Result intake rejects rows that omit the base envelope fields above or encode
  required numeric metrics as strings before provenance, plotting, or closeout
  checks can count them.
- Do not write a `paper-denominator-reproduction` row for a subset, single seed,
  deterministic fixture, or mechanics-only gate.
- Paper-denominator classification checks apply before overlay handling:
  `paper-denominator-reproduction` rows must name a paper model and `vLLM`
  even if `plot_binding` is temporarily null.
- Do not write success rows for SpreadsheetBench cases when the score envelope
  exists but the output workbook artifact is missing.
- Use `plot_binding: null` for real Leaven results whose denominator cannot be
  shown honestly on the paper-target panels.
- Do not silently repair denominator drift in the plotter. If `plot_binding`
  does not match a displayed paper target label, validation must fail.
- Overlay rows must use a `plot_binding.x_label` that exists in the committed
  target-plot provenance for the named panel; closeout cannot count an overlay
  row that the plotter would later reject.
- When an overlay label names a model family such as `122B` or `35B`, known
  Qwen paper model ids must match that family; a 35B row cannot be overlaid on
  a 122B paper target label.
- Artifact paths and `skill_source.path` values must be repo-relative,
  inspectable files at validation time. When `skill_source.path` is present,
  the same path must also appear in `artifact_paths`; a file-backed optimized
  skill is part of the row's artifact audit, not just metadata.
- `mechanics-smoke` and `deterministic-one-case` rows must always use
  `plot_binding: null`.
- `model-one-case`, `evolving-split-run`, and `training-validation-candidate`
  rows must also use `plot_binding: null`; they are real execution evidence but
  not paper-target overlays.
- Overlay rows must use units compatible with their panel: percent for score
  axes, minutes for runtime axes, and delta points for average-improvement
  axes.
- Approval-gated rows must carry at least one inspectable
  `extra.approval_artifact_paths` entry and that same path must be included in
  `artifact_paths`.
- A committed top-level `results/*.jsonl` row for an approval-gated proof
  classification must also have a runnable approval packet in
  `results/full_run_plan.md`. An inspectable approval artifact path proves
  provenance only; it is not approval while the packet is blocked.
- Approval-gated rows must also carry every prompt artifact required by their
  `extra.runbook_stage_id` in `full_denominator_runbook.json`, such as rendered
  agent prompts, Stage 2 analyst prompt/fanout files, Stage 3 merge prompts, or
  prompt-render manifests.
- Model-backed run rows must also carry every file-shaped artifact required by
  their generated runbook stage, such as prompt manifests, official evaluator
  outputs, parsed analysis JSON, fanout JSONL, merge manifests, change logs, or
  selection notes. Directory placeholders and the row's own
  `leaven_results.jsonl` are not used as artifact-path requirements.
- Result rows must carry `extra.runbook_stage_id`; result intake rejects rows
  whose stage is missing from the generated full-denominator runbook or whose
  stage `allowed_label` differs from the row's `proof_classification`.
- Result rows must also keep `dataset_slice` consistent with the named runbook
  stage's generated `expected_dataset_slice`: one-case stages stay one-case,
  one-case stages with a generated `case_id` must carry the exact matching
  `extra.case_id` such as `13-1`, every stage with a generated split must use
  that exact `dataset_slice.split`, subset rows must be held-out subsets below
  the 200-case paper denominator, G3/G3V use `0..200`, G4 uses `200..400`, and
  aggregate or full-paper rows use their generated case ranges, case counts,
  and explicit denominator labels.
- Official SpreadsheetBench evaluator imports additionally check the
  `eval_official_results.json` per-case `results[*].id` list against the
  declared `dataset_slice.case_range` in upstream dataset order. A row cannot
  claim `200..400`, `200..202`, or `0..200` from summary metrics alone.
- Paper-denominator-class rows (`held-out-single-seed-candidate`,
  `seed-aggregate-candidate`, `paper-denominator-candidate`, and
  `paper-denominator-reproduction`) must use a paper model id
  (`Qwen3.5-122B-A10B` or `Qwen3.5-35B-A3B`) and `vLLM`; fixture-model rows
  cannot count as held-out, seed-aggregate, or full-paper evidence.
- Result rows must also satisfy the named runbook stage's generated
  `expected_seed_policy`: model one-case and subset rows use seed `41`,
  evolving/validation/held-out single-seed rows use one of `41`, `42`, or `43`,
  and aggregate/full-paper rows carry `extra.seeds: [41, 42, 43]`.
- Result rows must also satisfy the named runbook stage's generated
  `expected_runtime_policy`: upstream run rows carry the paper worker count and
  ReAct turn budget, and skill-evolution rows additionally carry
  `extra.merge_batch_size: 32`.
- Result rows must also satisfy the named runbook stage's generated
  `expected_command_policy`: `extra.command_policy` must match the generated
  policy kind, and `source_command` must include the upstream command fragments
  required for that stage. When the row has a `dataset_slice.case_range`,
  `source_command` must also include matching concrete `--start_idx` and
  `--end_idx` fragments.
- Official evaluator-derived rows must keep `extra.source_metric` bound to the
  metric actually being counted. Non-overlay official rows use the canonical
  `official_*` metric names, and overlay rows may bind raw official metrics
  only to compatible score panels. Raw evaluator metrics cannot be relabeled as
  derived average-improvement or runtime overlays.
- Aggregate result rows must also satisfy the named runbook stage's generated
  `expected_aggregate_policy`: seed-aggregate rows must cite inspectable source
  result JSONL rows whose held-out single-seed predecessor rows pass result
  intake and cover seeds `41`, `42`, and `43`, and the aggregate `metric_value`
  must equal the mean of the cited seed metric values. Every cited
  seed-aggregate source row must be a G4 held-out single-seed candidate.
  Full-paper rows must cite training-validation or seed-aggregate source result
  rows that also pass result intake and together cover the generated required
  split ranges such as `0..200` and `200..400`, and the full-paper
  `metric_value` must equal the case-count-weighted mean of those cited split
  metrics. Every cited full-paper source row must have a source proof
  classification named by the generated aggregate policy; held-out single-seed
  rows must feed seed aggregates before they can contribute to G6. Aggregate and
  full-paper source rows must match the parent row's `model_id`,
  `serving_backend`, `metric_name`, and `metric_unit`, so source rows from a
  different model, backend, or metric cannot support a paper-shaped aggregate.
  If a cited source row lives in top-level ARA `results/*.jsonl`, approval-gated
  source rows must also pass the runnable approval-packet preflight before they
  can support an aggregate or full-paper row.
