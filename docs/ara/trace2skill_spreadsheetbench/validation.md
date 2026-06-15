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
cargo test -p trace2skill_spreadsheetbench --test manifest --test run_artifacts --test patch_bridge --test patch_replay --test one_case --test cli --test workbook_score --test acp_external_worker
```

Result:

```text
PASS: 49 tests across manifest, run_artifacts, patch_bridge, patch_replay,
one_case, cli, workbook_score, and acp_external_worker.
```

Scope:

- Confirms the focused Leaven mechanics and one-case deterministic proof targets listed in `evidence/leaven_mechanics_tests.md`.
- Confirms the ACP external-worker test uses the current typed `LockedMethod::AgentRun` / `MethodPrimaryKind::AgentSession` public seam values.

Limit:

- This focused gate does not run live Qwen3.5 models, vLLM, Trace2Skill analyst calls, live hierarchical merge, held-out `200..400`, seeds `41/42/43`, or cross-model/cross-domain paper metrics.

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

- No `results/*.jsonl` files exist yet, so this proves the result envelope and validator path only. It does not prove a Leaven run.

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
