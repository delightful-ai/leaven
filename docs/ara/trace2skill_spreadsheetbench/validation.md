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
- Reproduction tolerance policy is not defined.
- Exploration dead ends need explicit `failure_mode` and `lesson` fields.
- Prompt-template evidence remains missing.

Limit:

- The review is semantic judgment over the ARA contents after Level 1 validation. It does not execute the paper run.
