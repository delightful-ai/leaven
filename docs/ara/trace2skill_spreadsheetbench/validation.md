# Validation

## 2026-06-14 Seal Level 1 Structural Check

Command:

```bash
uv run --with pyyaml python scripts/validate_ara.py docs/ara/trace2skill_spreadsheetbench
```

Result:

```text
PASS: docs/ara/trace2skill_spreadsheetbench (23 files)
```

Scope:

- Confirms mandatory ARA directories and files exist and are non-empty.
- Confirms `PAPER.md` frontmatter and Layer Index exist.
- Confirms claims, experiments, concepts, heuristics, code refs, trace nodes, and evidence `Source` fields satisfy the local Seal Level 1 validator.

Limit:

- This does not prove source coverage completeness, full paper reproduction, Leaven result overlays, one-case live proof, held-out split execution, seed aggregation, or Qwen/vLLM parity.

## 2026-06-14 Plot Target Generation

Command:

```bash
uv run --with matplotlib --with pandas python scripts/plot_trace2skill_ara.py docs/ara/trace2skill_spreadsheetbench
```

Result:

```text
docs/ara/trace2skill_spreadsheetbench/plots/trace2skill_targets.png
```

Scope:

- Reads Markdown tables in `evidence/tables/`.
- Generates a paper target sheet with four panels.

Limit:

- The generated PNG contains paper target values only. It is not a Leaven reproduced result.

## 2026-06-14 Updated Seal Level 1 Check

Command:

```bash
uv run --with pyyaml python scripts/validate_ara.py docs/ara/trace2skill_spreadsheetbench
```

Result:

```text
PASS: docs/ara/trace2skill_spreadsheetbench (26 files)
```

## 2026-06-14 Focused Leaven Mechanics Gate

Command:

```bash
cargo test -p trace2skill_spreadsheetbench --test manifest --test run_artifacts --test patch_bridge --test patch_replay --test one_case --test one_case_run --test cli --test workbook_score --test acp_external_worker
```

Result:

```text
PASS: 53 tests across manifest, run_artifacts, patch_bridge, patch_replay,
one_case, one_case_run, cli, workbook_score, and acp_external_worker.
```

Scope:

- Confirms the focused Leaven mechanics and one-case deterministic proof targets listed in `evidence/leaven_mechanics_tests.md`.
- Confirms the ACP external-worker test uses the current typed `LockedMethod::AgentRun` / `MethodPrimaryKind::AgentSession` public seam values.

Limit:

- This focused gate does not run live Qwen3.5 models, vLLM, Trace2Skill analyst calls, live hierarchical merge, held-out `200..400`, seeds `41/42/43`, or cross-model/cross-domain paper metrics.

## 2026-06-14 Mechanics Classification Coverage Check

Artifact:

```text
scripts/validate_ara.py
docs/ara/trace2skill_spreadsheetbench/evidence/leaven_mechanics_tests.md
```

Command:

```bash
uv run --with pyyaml python scripts/validate_ara.py docs/ara/trace2skill_spreadsheetbench
```

Current result:

- Validator requires every current `examples/trace2skill_spreadsheetbench/tests/{manifest,run_artifacts,patch_bridge,patch_replay,one_case,one_case_run,cli,workbook_score,acp_external_worker}.rs` target to exist.
- Validator also requires `evidence/leaven_mechanics_tests.md` to classify each corresponding `cargo test -p trace2skill_spreadsheetbench --test <target>` command.

Limit:

- This prevents unclassified mechanics-test drift. It does not prove live
  Qwen/vLLM execution or promote any mechanics test to paper-denominator
  evidence.

## 2026-06-14 Paper Table Fidelity Check

Artifact:

```text
scripts/check_trace2skill_table_fidelity.py
scripts/validate_ara.py
docs/ara/trace2skill_spreadsheetbench/evidence/tables/*.md
tmp/skill_opt_sources/arx_2603.25158/src/tables/*.tex
```

Command:

```bash
uv run --with pyyaml python scripts/check_trace2skill_table_fidelity.py docs/ara/trace2skill_spreadsheetbench
```

Current result:

```text
PASS: docs/ara/trace2skill_spreadsheetbench table fidelity (6 tables)
```

Scope:

- Compares ordered body cells in six ARA Markdown evidence tables against the
  corresponding paper TeX source tables.
- Covers `table_main_v1`, `table_seq_parallel`, `table_reasoning_bank`,
  `table_agentic_ablation`, `table_math`, and `table_vqa`.
- The same check now runs inside `scripts/validate_ara.py`.

Limit:

- This proves table-cell transcription fidelity only. It does not prove Leaven
  reproduced any paper target value.

## 2026-06-14 Prompt Index Fidelity Check

Artifact:

```text
scripts/check_trace2skill_prompt_index.py
scripts/validate_ara.py
docs/ara/trace2skill_spreadsheetbench/evidence/prompt_templates.md
tmp/repros/trace2skill-upstream/
```

Command:

```bash
uv run --with pyyaml python scripts/check_trace2skill_prompt_index.py docs/ara/trace2skill_spreadsheetbench
```

Current result:

```text
PASS: docs/ara/trace2skill_spreadsheetbench prompt index (5 families)
```

Scope:

- Checks the five prompt families indexed in `prompt_templates.md` against the
  local upstream Trace2Skill checkout.
- Verifies family counts for spreadsheet system prompts, error-evolving
  prompts, success/combined evolving prompts, parallel merge/application
  prompts, and released skill files.
- Verifies every representative path in the index is locally inspectable.
- The same check now runs inside `scripts/validate_ara.py`.

Limit:

- This proves prompt-index fidelity only. It does not copy full prompts into
  the ARA or prove rendered live-call prompts, model execution, analyst output,
  or merge behavior.

## 2026-06-14 Result Overlay Schema Check

Command:

```bash
uv run --with pyyaml python scripts/validate_ara.py docs/ara/trace2skill_spreadsheetbench
```

Result:

```text
PASS: docs/ara/trace2skill_spreadsheetbench (31 files)
```

Scope:

- Confirms `results/README.md` and `results/leaven_result_schema.md` exist.
- Confirms any future `results/*.jsonl` rows satisfy the local result schema.
- Confirms the ARA still passes the structural Seal Level 1 checks.

Limit:

- At the time of this schema-only check, no `results/*.jsonl` files existed,
  so it proved the result envelope and validator path only. Current result-row
  state is checked by the later deterministic one-case and status-doc
  consistency sections.

## 2026-06-14 Plot Target Generation With Empty Result Overlay Set

Command:

```bash
uv run --with matplotlib --with pandas python scripts/plot_trace2skill_ara.py docs/ara/trace2skill_spreadsheetbench
```

Result:

```text
docs/ara/trace2skill_spreadsheetbench/plots/trace2skill_targets.png
```

Scope:

- Regenerates the paper target sheet.
- Confirms the plotter tolerates an empty `results/*.jsonl` set and preserves paper-target-only rendering.

Limit:

- This remains a paper target sheet until real Leaven result records are added.

## 2026-06-14 Plot Freshness Check

Artifact:

```text
scripts/check_trace2skill_plot_freshness.py
docs/ara/trace2skill_spreadsheetbench/plots/trace2skill_targets.png
```

Command:

```bash
uv run --with pyyaml python scripts/check_trace2skill_plot_freshness.py docs/ara/trace2skill_spreadsheetbench
```

Current result:

```text
PASS: docs/ara/trace2skill_spreadsheetbench plot freshness
```

Scope:

- Regenerates `plots/trace2skill_targets.png` into a temporary directory by
  invoking `scripts/plot_trace2skill_ara.py` with `uv run --with matplotlib
  --with pandas`.
- Compares the committed PNG SHA-256 to the temporary render.
- The integrated Seal Level 1 validator now runs this freshness check after
  the plot-provenance check.

Limit:

- This proves the checked-in paper target plot is freshly renderable from
  current ARA evidence and current result-row state. It does not create Leaven
  overlays, approve Qwen/vLLM execution, or prove any paper-denominator result.

## 2026-06-14 Temporary Overlay Parser Smoke

Command:

```bash
uv run --with matplotlib --with pandas python scripts/plot_trace2skill_ara.py docs/ara/trace2skill_spreadsheetbench --results /tmp/trace2skill-overlay-smoke.jsonl --output /tmp/trace2skill-overlay-smoke.png
```

Result:

```text
PASS: temporary overlay PNG was written outside the repo.
```

Scope:

- Confirms one valid result-shaped JSONL row can bind to a displayed plot label and render as an overlay marker.

Limit:

- The smoke row was temporary, classified `mechanics-smoke`, and was not stored in the ARA. It is not a Leaven result.

## 2026-06-14 Runbook Freshness Check

Artifact:

```text
scripts/check_trace2skill_runbook_freshness.py
scripts/build_trace2skill_runbook.py
docs/ara/trace2skill_spreadsheetbench/results/full_denominator_runbook.{json,md}
```

Command:

```bash
uv run --with pyyaml python scripts/check_trace2skill_runbook_freshness.py docs/ara/trace2skill_spreadsheetbench
```

Current result:

```text
PASS: docs/ara/trace2skill_spreadsheetbench runbook freshness
```

Scope:

- Regenerates the full-denominator runbook into a temporary directory.
- Verifies the committed JSON and Markdown runbook outputs match the generator exactly.
- The same check now runs inside `scripts/validate_ara.py`.

Limit:

- This proves generated runbook freshness only. It does not approve the packet, fill unresolved model/hardware/cost fields, launch Qwen/vLLM, or create paper-denominator result rows.

## 2026-06-14 Artifact Contract Check

Artifact:

```text
scripts/check_trace2skill_artifact_contract.py
docs/ara/trace2skill_spreadsheetbench/results/full_run_plan.md
docs/ara/trace2skill_spreadsheetbench/results/full_denominator_runbook.json
```

Command:

```bash
uv run --with pyyaml python scripts/check_trace2skill_artifact_contract.py docs/ara/trace2skill_spreadsheetbench
```

Current result:

```text
PASS: docs/ara/trace2skill_spreadsheetbench artifact contract
```

Scope:

- Verifies the approval packet still expects normalized run metadata, dataset manifest, rendered agent prompts, trajectories, score reports, Stage 2 analyst prompt fanout, Stage 3 merge prompts/manifests, skill-evolution artifacts, held-out eval artifacts, prompt-render manifests, and `leaven_results.jsonl`.
- Verifies every generated runbook stage lists the artifact fragments needed for its stated denominator, including one-case manifest/transcript/rendered Stage 2 evidence, future rendered prompt manifests, and future Leaven result JSONL outputs for model-backed/subset/held-out stages.
- The same check now runs inside `scripts/validate_ara.py`.

Limit:

- This proves artifact-expectation coverage only. It does not prove those future artifacts exist, approve model/hardware/cost, import result rows, draw overlays, or execute Qwen/vLLM.

## 2026-06-14 Deterministic One-Case ACP Worker Run

Prepare command:

```bash
cargo run -p trace2skill_spreadsheetbench -- --prepare-one-case-run --run-dir tmp/trace2skill-one-case-live
```

Run and score command:

```bash
cargo run -p trace2skill_spreadsheetbench -- --run-one-case-acp-worker --run-dir tmp/trace2skill-one-case-live --model-id local-openpyxl-trace2skill-agent
```

Result:

```text
PASS: score 1.0, matched 120/120 cells, output workbook 8423 bytes.
```

Artifacts:

- `tmp/trace2skill-one-case-live/13-1_output.xlsx`
- `tmp/trace2skill-one-case-live/acp_result.json`
- `tmp/trace2skill-one-case-live/agent_transcript.md`
- `tmp/trace2skill-one-case-live/manifest.json`
- `tmp/trace2skill-one-case-live/score_report.json`
- `tmp/trace2skill-one-case-live/trajectory.json`

Scope:

- Confirms the promoted CLI path can prepare a real `13-1` run, dispatch a deterministic local Python worker through `leaven/agent.run`, require a workbook artifact in the ACP result, score the workbook, and write trajectory evidence.

Limit:

- This is `deterministic-one-case` evidence only. It does not use Qwen3.5, vLLM, Trace2Skill analyst calls, live hierarchical merge, held-out `200..400`, seeds `41/42/43`, or paper aggregate metrics.

## 2026-06-14 Full Run Approval Gate

Artifact:

```text
docs/ara/trace2skill_spreadsheetbench/results/full_run_plan.md
```

Scope:

- Records the required model, vLLM, dataset, split, seed, worker, merge, turn-budget, cost, credential, and artifact-retention approvals before any Qwen/vLLM-scale run.
- Defines subset gates from one case through small `N`, evolving rows `0..200`, held-out rows `200..400`, seed aggregation, and cross-model/condition coverage.

Limit:

- The approval packet is not filled or approved. No full-denominator execution has been launched.

## 2026-06-14 Seal Level 2 Semantic Rigor Review

Artifacts:

```text
docs/ara/trace2skill_spreadsheetbench/level2_report.json
docs/ara/trace2skill_spreadsheetbench/reviews/rigor_review.md
```

Result:

```text
Weak Accept: usable as an anti-proxy denominator package, not as reproduction proof.
```

Top blockers:

- Full-run approval fields are unresolved.
- Full-run approval fields are unresolved.
- Proposed reproduction tolerance exists but is not approved.
- Prompt-template families are indexed, but exact rendered live-call prompts remain future run artifacts.

Limit:

- The review is semantic judgment over the ARA contents after Level 1 validation. It does not execute the paper run.

## 2026-06-14 Post-Review Follow-Ups

Artifacts:

```text
docs/ara/trace2skill_spreadsheetbench/src/configs/tolerance.md
docs/ara/trace2skill_spreadsheetbench/evidence/prompt_templates.md
docs/ara/trace2skill_spreadsheetbench/trace/exploration_tree.yaml
docs/ara/trace2skill_spreadsheetbench/logic/claims.md
```

Scope:

- Adds a proposed per-metric/runtime/protocol-drift tolerance policy.
- Indexes upstream prompt-template families.
- Adds `failure_mode` and `lesson` fields to the two dead-end exploration nodes.
- Links C07 to E09 so the full-run approval gate is part of closeout proof.

Limit:

- The tolerance policy is proposed, not approved. Full Qwen/vLLM execution remains blocked on the approval packet.

## 2026-06-14 Denominator Status Audit

Artifact:

```text
docs/ara/trace2skill_spreadsheetbench/results/denominator_status.md
```

Scope:

- Compares the current ARA state to every acceptance item in
  `docs/working-memory/trace2skill-ara-reproduction-goal-handoff.yaml`.
- States the strongest currently reproduced denominator as deterministic
  one-case ACP worker proof.
- Names the unresolved approval blocker for full Qwen/vLLM paper-denominator
  execution.

Limit:

- This is a status audit, not a final closeout. It does not run held-out rows,
  seeds `41/42/43`, or cross-model paper metrics.

## 2026-06-14 Model Availability Research

Artifact:

```text
docs/ara/trace2skill_spreadsheetbench/results/model_availability.md
```

Scope:

- Records that public `Qwen/Qwen3.5-122B-A10B` and `Qwen/Qwen3.5-35B-A3B`
  model repositories exist.
- Records documented vLLM/OpenAI-compatible serving paths and the 122B hardware
  sizing note found during availability research.
- Maps the local upstream reproduction hooks for model name, seeds,
  generation configs, workers, split indices, and evolution worker settings.

Limit:

- This does not provision a model endpoint, approve cost, approve hardware,
  create credentials, or launch a Qwen/vLLM run.

## 2026-06-14 Approval Packet Preflight

Artifact:

```text
scripts/check_trace2skill_approval_packet.py
docs/ara/trace2skill_spreadsheetbench/results/full_run_plan.md
```

Command:

```bash
uv run --with pyyaml python scripts/check_trace2skill_approval_packet.py docs/ara/trace2skill_spreadsheetbench --expect-blocked
```

Scope:

- Extracts the fenced YAML approval packet from `results/full_run_plan.md`.
- Verifies fixed paper-denominator fields: vLLM backend, SpreadsheetBench path,
  seeds `41/42/43`, workers `128`, merge batch size `32`, and ReAct turn budget
  `100`.
- Requires concrete approval values for model endpoints/weights, hardware,
  dataset checksum or manifest, cost, credentials, retention, and approval
  metadata before normal preflight can pass.
- Requires the expected artifact list to include run metadata, manifests,
  trajectories, score reports, evolved skill output, and `leaven_results.jsonl`.

Limit:

- `--expect-blocked` is a guardrail proof only. It confirms that the current
  packet is not runnable; it is not reproduction evidence.

## 2026-06-14 Approval State Consistency Check

Artifact:

```text
scripts/check_trace2skill_approval_state.py
docs/ara/trace2skill_spreadsheetbench/results/full_run_plan.md
docs/ara/trace2skill_spreadsheetbench/results/closeout_audit.json
docs/ara/trace2skill_spreadsheetbench/results/closeout_audit.md
docs/ara/trace2skill_spreadsheetbench/results/denominator_status.md
```

Command:

```bash
uv run --with pyyaml python scripts/check_trace2skill_approval_state.py docs/ara/trace2skill_spreadsheetbench
```

Current result:

```text
PASS: docs/ara/trace2skill_spreadsheetbench approval state
```

Scope:

- Reuses the approval-packet parser and policy check.
- If the packet is blocked, verifies the closeout audit reports
  `full_denominator_plan_approved` as blocked, carries each packet blocker, and
  keeps `overall_complete` false.
- Verifies human status docs still state that normal approval preflight is
  blocked and Qwen/vLLM-scale execution must not launch.
- If the packet ever becomes runnable, the same checker rejects stale blocked
  language in closeout/status docs.
- The integrated Seal Level 1 validator now runs this approval-state check
  before runbook and closeout freshness checks.

Limit:

- This proves approval-state consistency only. It does not resolve the approval
  fields, provision model endpoints, create credentials, approve tolerance, or
  execute any model-backed Trace2Skill denominator.

## 2026-06-14 Dataset Manifest

Artifact:

```text
docs/ara/trace2skill_spreadsheetbench/results/dataset_manifest.json
scripts/build_trace2skill_dataset_manifest.py
```

Command:

```bash
uv run python scripts/build_trace2skill_dataset_manifest.py
```

Recorded facts:

- `dataset.json` has 400 records and SHA-256
  `bcecaa89a005bd4e3bbe98da150a86e8062c27f262e575d5e47bd9861b3525e7`.
- Ordered case ids run from `13-1` to `59902`, with full-order SHA-256
  `ac05d2035ad776af9d901689423645316e707e6e8426a04d2eae6591929b64e9`.
- Evolving split `0..200` runs from `13-1` to `52575`.
- Held-out split `200..400` runs from `52807` to `59902`.
- Referenced spreadsheet directories are all present; the aggregate workbook
  hash covers 2,394 files.

Limit:

- This manifests local data provenance and split materialization only. It does
  not approve a Qwen/vLLM run and does not produce any Leaven metric rows.

## 2026-06-14 Dataset Manifest Freshness Check

Artifact:

```text
scripts/check_trace2skill_dataset_manifest_freshness.py
scripts/build_trace2skill_dataset_manifest.py
docs/ara/trace2skill_spreadsheetbench/results/dataset_manifest.json
```

Command:

```bash
uv run --with pyyaml python scripts/check_trace2skill_dataset_manifest_freshness.py docs/ara/trace2skill_spreadsheetbench
```

Current result:

```text
PASS: docs/ara/trace2skill_spreadsheetbench dataset manifest freshness
```

Scope:

- Rebuilds `dataset_manifest.json` into a temporary directory from the current
  local upstream SpreadsheetBench data.
- Compares the temporary manifest byte-for-byte with the committed manifest.
- The integrated Seal Level 1 validator now runs this freshness check after the
  protocol/config fidelity check.

Limit:

- This proves only that Leaven's recorded 400-row dataset provenance and split
  manifest are current. It does not approve model execution, inspect held-out
  results, create overlays, or reproduce any Trace2Skill paper metric.

## 2026-06-14 Closeout Audit

Artifact:

```text
scripts/audit_trace2skill_closeout.py
docs/ara/trace2skill_spreadsheetbench/results/closeout_audit.md
docs/ara/trace2skill_spreadsheetbench/results/closeout_audit.json
```

Command:

```bash
uv run --with pyyaml python scripts/audit_trace2skill_closeout.py docs/ara/trace2skill_spreadsheetbench
```

Current result:

- `overall_complete` is `false`.
- Current denominators are limited to paper-target capture, mechanics-test
  classification, and deterministic one-case `13-1`.
- Missing denominators include model-backed one-case, small-N subset,
  evolving split `0..200`, held-out split `200..400`, seed aggregate
  `41/42/43`, cross-model paper rows, and full paper denominator.

Limit:

- This is a closeout guardrail, not a reproduction result. It is expected to
  remain incomplete until approved Qwen/vLLM runs produce denominator-labeled
  Leaven result records.

## 2026-06-14 Closeout Freshness Check

Artifact:

```text
scripts/check_trace2skill_closeout_freshness.py
scripts/audit_trace2skill_closeout.py
docs/ara/trace2skill_spreadsheetbench/results/closeout_audit.{json,md}
```

Command:

```bash
uv run --with pyyaml python scripts/check_trace2skill_closeout_freshness.py docs/ara/trace2skill_spreadsheetbench
```

Current result:

```text
PASS: docs/ara/trace2skill_spreadsheetbench closeout freshness
```

Scope:

- Regenerates the closeout audit into a temporary directory.
- Verifies committed JSON and Markdown closeout outputs exactly match current
  approval, result-row, dataset-manifest, and deterministic one-case state.
- Runs result-intake validation before closeout row counting and records
  `result_intake_summary.valid: true` plus the checker path in the closeout
  artifacts.
- The same check now runs inside `scripts/validate_ara.py`.

Limit:

- This proves generated closeout freshness only. It does not make
  `overall_complete` true, approve model/hardware/cost, create result rows,
  draw overlays, or execute Qwen/vLLM.

## 2026-06-14 Full-Denominator Runbook

Artifact:

```text
scripts/build_trace2skill_runbook.py
docs/ara/trace2skill_spreadsheetbench/results/full_denominator_runbook.md
docs/ara/trace2skill_spreadsheetbench/results/full_denominator_runbook.json
```

Command:

```bash
uv run --with pyyaml python scripts/build_trace2skill_runbook.py docs/ara/trace2skill_spreadsheetbench
```

Current result:

- Normal approval preflight is `false`.
- Runnable-now stages are limited to no-spend guardrails and deterministic
  one-case Leaven seam proof.
- Approval-required stages are model-backed one-case, small-N subset, evolving
  split `0..200`, training-set validation, held-out split `200..400`, seed
  aggregate, and cross-model/full paper rows.

Limit:

- This is an execution map and artifact checklist. It does not approve model
  work and does not create Leaven metric rows.

## 2026-06-14 Deterministic One-Case Result JSONL

Artifact:

```text
scripts/build_trace2skill_one_case_result.py
docs/ara/trace2skill_spreadsheetbench/results/deterministic_one_case.jsonl
```

Command:

```bash
uv run python scripts/build_trace2skill_one_case_result.py
```

Current result:

- One row with `proof_classification: deterministic-one-case`.
- `metric_name: workbook_score`, `metric_value: 1.0`, and
  `metric_unit: fraction`.
- `plot_binding` is `null`, so the paper-target plotter validates the record
  but does not overlay it on SpreadsheetBench/Qwen/vLLM target panels.

Limit:

- This is a real Leaven result record for the deterministic one-case seam proof,
  not held-out, seed-aggregate, cross-model, or full paper reproduction
  evidence.

## 2026-06-14 Official Eval Result Importer

Artifact:

```text
scripts/import_trace2skill_eval_results.py
scripts/check_trace2skill_importer_fixture.py
scripts/fixtures/trace2skill_eval_official_results_sample.json
```

Command:

```bash
uv run --with pyyaml python scripts/import_trace2skill_eval_results.py \
  --eval-results scripts/fixtures/trace2skill_eval_official_results_sample.json \
  --output tmp/trace2skill-import-fixture/imported.jsonl \
  --ara-dir docs/ara/trace2skill_spreadsheetbench \
  --run-id trace2skill-import-fixture \
  --created-at 2026-06-14T00:00:02Z \
  --proof-classification paper-subset \
  --runbook-stage-id G2 \
  --split fixture \
  --case-range 0..2 \
  --case-count 2 \
  --denominator fixture-only-not-paper \
  --model-id fixture-model \
  --serving-backend fixture-backend \
  --seed 41 \
  --skill-kind fixture-skill \
  --artifact-path target/trace2skill-import-fixture/subset_0_2_seed_41/rendered_prompts/13-1/agent_prompt.md \
  --artifact-path target/trace2skill-import-fixture/subset_0_2_seed_41/prompt_render_manifest.json \
  --approval-artifact-path docs/ara/trace2skill_spreadsheetbench/results/full_run_plan.md \
  --source-command 'fixture importer smoke'
```

Current result:

- Integrated ARA validation now runs the importer fixture checker:

```bash
uv run --with pyyaml python scripts/check_trace2skill_importer_fixture.py docs/ara/trace2skill_spreadsheetbench
```

- Writes four rows for the official evaluator summary metrics:
  `official_instance_accuracy`, `official_test_case_accuracy`,
  `official_avg_soft_score`, and `official_avg_hard_score`.
- Converts evaluator fractions to percent-valued Leaven result rows.
- Defaults every row to `plot_binding: null`.
- Runs the result-intake checker before writing output rows.
- Requires every row to carry `extra.runbook_stage_id` and validates the stage
  label against `full_denominator_runbook.json` before write and during result
  intake.
- Requires approval-gated rows to include prompt artifact paths matching their
  runbook stage expectations, such as rendered prompts and prompt-render
  manifests.
- Requires approval-gated proof classifications, including `paper-subset`, to
  include at least one inspectable `--approval-artifact-path`.
- Requires `--eval-results`, `--skill-path`, `--artifact-path`, and
  `--approval-artifact-path` values to be locally inspectable when provided.
- Refuses `paper-denominator-reproduction` unless
  `--allow-paper-denominator-reproduction` and at least one
  `--approval-artifact-path` are explicitly present.
- The checker exercises those refusal paths directly: missing runbook stage id,
  wrong runbook stage label, missing prompt artifacts for `paper-subset`,
  missing approval evidence for `paper-subset`, and accidental
  `paper-denominator-reproduction` without the explicit allow flag.

Limit:

- The fixture is script coverage only. It is not stored under top-level
  `results/*.jsonl`, not plotted, and not evidence for paper reproduction.

## 2026-06-14 Paper Figure Fidelity Check

Artifact:

```text
scripts/check_trace2skill_figure_index.py
docs/ara/trace2skill_spreadsheetbench/evidence/figures/figure_trace2skill_framework.md
```

Command:

```bash
uv run --with pyyaml python scripts/check_trace2skill_figure_index.py docs/ara/trace2skill_spreadsheetbench
```

Current result:

- Verifies the ARA framework figure index points at the inspectable upstream
  `trace2skill_framwork.png` file.
- Checks the source identifier `fig:pipeline`, source path, byte size,
  SHA-256 digest, and PNG magic bytes.
- The integrated Seal Level 1 validator now runs this figure-fidelity check
  alongside paper table and prompt-index checks.

Limit:

- This proves source anchoring for the framework figure only. It does not prove
  plotted Leaven metrics, source-image visual equivalence beyond the file
  identity check, or any paper-denominator reproduction.

## 2026-06-14 Protocol Configuration Fidelity Check

Artifact:

```text
scripts/check_trace2skill_config_fidelity.py
docs/ara/trace2skill_spreadsheetbench/src/configs/training.md
docs/ara/trace2skill_spreadsheetbench/src/configs/model.md
docs/ara/trace2skill_spreadsheetbench/results/full_denominator_runbook.md
docs/ara/trace2skill_spreadsheetbench/results/full_denominator_runbook.json
```

Command:

```bash
uv run --with pyyaml python scripts/check_trace2skill_config_fidelity.py docs/ara/trace2skill_spreadsheetbench
```

Current result:

- Verifies the ARA config tables against the paper implementation paragraph,
  upstream README reproduction variables, dataset manifest, Qwen generation
  JSON files, and generated full-denominator runbook.
- Confirms the paper-denominator protocol values: 400 SpreadsheetBench rows,
  splits `0..200` and `200..400`, seeds `41/42/43`, 128 workers, merge batch
  size 32, ReAct turn budget 100, vLLM serving, and Qwen3.5 model/config IDs.
- Records the important upstream caveat: the upstream README omits
  `--merge-batch-size`, and the upstream script default is `5`; the Leaven
  runbook now passes `--merge-batch-size "$MERGE_BATCH_SIZE"` explicitly so an
  approved run does not silently use the non-paper merge tree.
- The integrated Seal Level 1 validator now runs this config-fidelity check
  alongside table, prompt, and figure fidelity checks.

Limit:

- This is protocol evidence and runbook hardening only. It does not launch
  Qwen/vLLM, create paper-denominator result rows, or prove that the configured
  run has been executed.

## 2026-06-14 Deterministic One-Case Artifact Check

Artifact:

```text
scripts/check_trace2skill_one_case_artifacts.py
docs/ara/trace2skill_spreadsheetbench/results/one_case_live.md
docs/ara/trace2skill_spreadsheetbench/results/deterministic_one_case.jsonl
tmp/trace2skill-one-case-live/
```

Command:

```bash
uv run --with pyyaml python scripts/check_trace2skill_one_case_artifacts.py docs/ara/trace2skill_spreadsheetbench
```

Current result:

- Verifies the deterministic one-case run directory has the expected manifest,
  ACP result, transcript, score report, trajectory, worker script, and output
  workbook.
- Confirms case `13-1` scored `1.0` with `120/120` matched cells and no
  mismatches.
- Confirms the output workbook is `8423` bytes with SHA-256
  `131cf073e40f73b5f152d3a4d718532ee6c980e467e48e1a136e1275cd31bf40`, and that
  the ACP receipt reports the same workbook digest.
- Confirms `deterministic_one_case.jsonl` remains a single
  `deterministic-one-case` record with denominator `one-case-13-1-only` and
  `plot_binding: null`.
- The integrated Seal Level 1 validator and closeout audit now use this checker
  before treating the deterministic one-case denominator as satisfied.

Limit:

- This proves the local deterministic ACP one-case denominator only. It does
  not prove model-backed Qwen/vLLM execution, held-out rows `200..400`, seed
  aggregation, cross-model rows, or full-paper reproduction.

## 2026-06-14 Deterministic One-Case Result Freshness Check

Artifact:

```text
scripts/check_trace2skill_one_case_result_freshness.py
scripts/build_trace2skill_one_case_result.py
docs/ara/trace2skill_spreadsheetbench/results/deterministic_one_case.jsonl
tmp/trace2skill-one-case-live/
```

Command:

```bash
uv run --with pyyaml python scripts/check_trace2skill_one_case_result_freshness.py docs/ara/trace2skill_spreadsheetbench
```

Current result:

```text
PASS: docs/ara/trace2skill_spreadsheetbench deterministic one-case result freshness
```

Scope:

- Rebuilds `deterministic_one_case.jsonl` into a temporary directory from the
  current deterministic one-case manifest, ACP receipt, transcript, score
  report, trajectory, output workbook, and worker script.
- Compares the temporary JSONL byte-for-byte with the committed result row.
- The integrated Seal Level 1 validator now runs this freshness check after the
  deterministic one-case artifact check.

Limit:

- This proves the stored deterministic one-case result row is fresh with
  respect to current local one-case artifacts. It does not make the row a
  paper-target overlay, model-backed one-case, held-out split, seed aggregate,
  or full Trace2Skill reproduction.

## 2026-06-14 Stage 2 Rendered Prompt Artifacts

Artifact:

```text
scripts/check_trace2skill_stage2_prompt_artifacts.py
docs/ara/trace2skill_spreadsheetbench/evidence/stage2_rendered_prompts.md
tmp/trace2skill-one-case-live/stage2_analyst_prompt.md
tmp/trace2skill-one-case-live/stage2_fanout.json
```

Command:

```bash
uv run --with pyyaml python scripts/check_trace2skill_stage2_prompt_artifacts.py docs/ara/trace2skill_spreadsheetbench
```

Current result:

- Regenerates the deterministic one-case Stage 2 prompt/fanout artifacts before
  checking their content.
- Verifies the rendered one-case Stage 2 MAP analyst prompt exists and has
  SHA-256
  `94893fef2c3459bbe76bb63854dd2e9aab813625877c584867d34eadba700ba4`.
- Verifies the rendered prompt embeds exact upstream source-template text for
  the success-path Stage 2 prompt files under
  `tmp/repros/trace2skill-upstream/skill_evolver/prompts/`.
- Verifies `stage2_fanout.json` has exactly one pending `Success` call,
  `success-13-1-1`, with `response: null`, `retry_count: 0`, and a prompt
  BlobRef pointing at `stage2_analyst_prompt.md`.
- The integrated Seal Level 1 validator now runs this check alongside the
  deterministic one-case artifact and result-freshness checks.

Limit:

- This proves rendered-prompt and pending-fanout artifact fidelity only for the
  deterministic one-case path. It does not execute the analyst model call,
  parse a response, merge patches, approve tolerance, create overlays, or run
  Qwen/vLLM.

## 2026-06-14 Status Doc Consistency Check

Artifact:

```text
scripts/check_trace2skill_status_docs.py
docs/ara/trace2skill_spreadsheetbench/results/denominator_status.md
docs/ara/trace2skill_spreadsheetbench/results/closeout_audit.json
docs/ara/trace2skill_spreadsheetbench/validation.md
```

Command:

```bash
uv run --with pyyaml python scripts/check_trace2skill_status_docs.py docs/ara/trace2skill_spreadsheetbench
```

Current result:

- Verifies `denominator_status.md` uses the current ARA file count from
  `validate_ara.py`.
- Verifies the human status docs match the current result JSONL state: one
  JSONL file, one total row, zero overlay rows, and zero paper-denominator rows.
- Verifies `closeout_audit.json` reports the same result-row summary as the
  actual `results/*.jsonl` files.
- The integrated Seal Level 1 validator now runs this status-doc check after
  result intake and evidence-binding validation.

Limit:

- This prevents stale status prose from contradicting current artifacts. It does
  not add Leaven result rows, overlay metrics, or Qwen/vLLM paper-denominator
  execution.

## 2026-06-14 Rigor Review Follow-Up Check

Artifact:

```text
scripts/check_trace2skill_rigor_followup.py
docs/ara/trace2skill_spreadsheetbench/level2_report.json
docs/ara/trace2skill_spreadsheetbench/reviews/rigor_review.md
docs/ara/trace2skill_spreadsheetbench/trace/exploration_tree.yaml
docs/ara/trace2skill_spreadsheetbench/logic/claims.md
```

Command:

```bash
uv run --with pyyaml python scripts/check_trace2skill_rigor_followup.py docs/ara/trace2skill_spreadsheetbench
```

Current result:

- Verifies the Level 2 report's `post_review_followup` lists F02 and F04 as
  addressed, F03 and F05 as partially addressed, and keeps remaining blockers
  non-empty.
- Verifies every dead-end exploration node has non-empty `failure_mode` and
  `lesson` fields.
- Verifies C07 cites both E08 and E09.
- Verifies tolerance and prompt-template follow-up artifacts exist.
- Verifies the human rigor-review notes no longer contradict the addressed F02
  and F04 status.

Limit:

- This checks follow-up consistency for the existing semantic review. It does
  not rerun a new independent Level 2 review, approve tolerance, capture
  rendered live prompts, or execute Qwen/vLLM.

## 2026-06-14 Runbook Label Consistency Check

Artifact:

```text
scripts/check_trace2skill_runbook_labels.py
docs/ara/trace2skill_spreadsheetbench/results/full_denominator_runbook.json
docs/ara/trace2skill_spreadsheetbench/results/README.md
docs/ara/trace2skill_spreadsheetbench/results/leaven_result_schema.md
```

Command:

```bash
uv run --with pyyaml python scripts/check_trace2skill_runbook_labels.py docs/ara/trace2skill_spreadsheetbench
```

Current result:

- Verifies every result-bearing runbook `allowed_label` is accepted by the
  JSONL schema, importer, plotter, and validator.
- Verifies staged denominator labels include model one-case, paper subset,
  evolving split, training validation, held-out single-seed, seed aggregate, and
  paper-denominator reproduction labels.
- Verifies model one-case, evolving split, and training validation rows remain
  non-overlay result rows.

Limit:

- This synchronizes denominator vocabulary only. It does not create any new
  result rows, approve the full run, or execute Qwen/vLLM.

## 2026-06-14 Prompt Source Manifest Check

Artifact:

```text
scripts/check_trace2skill_prompt_manifest.py
docs/ara/trace2skill_spreadsheetbench/evidence/prompt_templates.manifest.json
docs/ara/trace2skill_spreadsheetbench/evidence/prompt_templates.md
```

Command:

```bash
uv run --with pyyaml python scripts/check_trace2skill_prompt_manifest.py docs/ara/trace2skill_spreadsheetbench
```

Current result:

- Verifies every upstream prompt-template and released-skill prompt file is
  represented by repo-relative path, byte size, line count, and SHA-256.
- Confirms the ARA prompt evidence identifies the exact prompt source corpus
  without copying full prompt text into the ARA.
- The integrated Seal Level 1 validator now runs this manifest check after the
  prompt-family index check.

Limit:

- This proves source prompt identity only. It does not prove rendered prompts
  for live analyst calls, filled trajectory fields, model outputs, parser
  results, or Qwen/vLLM execution.

## 2026-06-14 Upstream Execution Code Manifest Check

Artifact:

```text
scripts/check_trace2skill_upstream_code_manifest.py
docs/ara/trace2skill_spreadsheetbench/src/execution/upstream_code_manifest.json
docs/ara/trace2skill_spreadsheetbench/results/full_denominator_runbook.json
```

Command:

```bash
uv run --with pyyaml python scripts/check_trace2skill_upstream_code_manifest.py docs/ara/trace2skill_spreadsheetbench
```

Current result:

- Verifies the upstream Python entrypoints used by the generated runbook are
  represented by repo-relative path, role, byte size, line count, and SHA-256.
- Verifies the generated runbook still references the pinned run, evaluation,
  analysis, parser, and parallel skill-evolution entrypoints.
- The integrated Seal Level 1 validator now runs this check after protocol
  config fidelity.

Limit:

- This proves source-code identity only. It does not run the entrypoints,
  validate their runtime environment, produce result rows, or execute Qwen/vLLM.
