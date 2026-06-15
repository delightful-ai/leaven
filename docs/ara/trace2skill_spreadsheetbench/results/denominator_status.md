# Denominator Status Audit

This audit compares the current ARA and Leaven artifacts to
`docs/working-memory/trace2skill-ara-reproduction-goal-handoff.yaml`.

It is not a closeout claim. The full Trace2Skill / SpreadsheetBench paper
denominator remains unreproduced until the approved Qwen/vLLM run produces
result records for the relevant held-out, seed-aggregate, and cross-model rows.

## Handoff Acceptance Status

| Acceptance id | Current status | Evidence | Remaining work |
|---------------|----------------|----------|----------------|
| `ara_level1_valid` | Satisfied for current ARA structure and source-bound evidence package. | `uv run --with pyyaml python scripts/validate_ara.py docs/ara/trace2skill_spreadsheetbench` passes with 50 files; `PAPER.md`, `logic/*`, `src/configs/*`, `trace/exploration_tree.yaml`, `evidence/*`, result records, closeout audit, runbook, and trace ledgers exist. | Manual evidence fidelity remains a continuing obligation when new tables/prompts/results are added. |
| `plots_from_ara` | Satisfied for paper target plots and overlay mechanism; no real paper-denominator overlay exists yet. | `plots/trace2skill_targets.png`; `scripts/plot_trace2skill_ara.py`; `results/leaven_result_schema.md`. | Add `results/*.jsonl` overlay rows only after a Leaven run produces metrics that bind honestly to paper target labels. |
| `current_mechanics_classified` | Satisfied for current focused mechanics and one-case tests. | `evidence/leaven_mechanics_tests.md`; `validation.md` focused gate; tests classify manifest, run artifacts, patch bridge/replay, one-case CLI/scorer, workbook scorer, and ACP external worker. | Re-run focused tests after changing the example crate or proof classifications. |
| `one_case_live_or_explicit_blocker` | Satisfied as deterministic one-case ACP worker proof, not model-backed paper parity. | `results/one_case_live.md`; `tmp/trace2skill-one-case-live/{13-1_output.xlsx,acp_result.json,agent_transcript.md,manifest.json,score_report.json,trajectory.json}`. | A model-backed one-case run remains separate future evidence if the user approves a live model path. |
| `full_denominator_plan_approved` | Planned but not approved; dataset manifest is recorded, blocked preflight is executable, and staged runbook is generated. | `results/full_run_plan.md`; `results/full_denominator_runbook.md`; `results/dataset_manifest.json`; `src/environment.md`; `src/configs/tolerance.md`; `scripts/check_trace2skill_approval_packet.py` with `--expect-blocked`. | Fill and approve model endpoints/weights, vLLM host/version, hardware, cost, credentials, tolerance approval, artifact retention, and stop conditions. |
| `reproduced_claim_limited_to_actual_denominator` | Partially satisfied as an active guardrail, not final closeout; executable closeout audit says incomplete. | `reviews/rigor_review.md`; `level2_report.json`; `results/closeout_audit.md`; `results/closeout_audit.json`; this audit; all result docs label current proof as target, mechanics, deterministic one-case, or approval-gated. | Final closeout must compare actual Leaven result JSONL overlays against paper targets and state the reproduced denominator exactly. |

## Current Reproduced Denominator

| Denominator | Status | Claim allowed |
|-------------|--------|---------------|
| Paper target tables | Captured and plotted. | "Paper target sheet generated from ARA evidence." |
| Mechanics tests | Classified and passing in the latest focused gate. | "Leaven mechanics for lowering/replay/scoring are covered." |
| Case `13-1` deterministic ACP worker | Run and scored with workbook artifact. | "Deterministic one-case seam proof." |
| Model-backed case `13-1` | Not run. | No model-backed one-case claim. |
| Small `N` subset | Not run. | No subset claim. |
| Evolving rows `0..200` | Not run. | No skill-evolution run claim. |
| Held-out rows `200..400` | Not run. | No held-out paper split claim. |
| Seeds `41/42/43` | Not run. | No seed aggregate claim. |
| Full paper denominator | Not run. | No 1:1 reproduction claim. |

## Approval Blocker

The remaining blocker is intentional and external to no-spend ARA work:

- Qwen3.5-122B-A10B availability is not approved.
- Qwen3.5-35B-A3B availability is not approved.
- vLLM serving shape, hardware, credentials, cost, artifact retention, and
  tolerance approval are not filled in `results/full_run_plan.md`.
- `scripts/check_trace2skill_approval_packet.py` is expected to fail in normal
  mode until these fields are approved; use `--expect-blocked` to verify the
  current no-launch state.

Do not launch Qwen/vLLM-scale execution or mark this goal complete until those
fields are resolved and approved.

## Current Result Record State

Current result JSONL state: 1 file(s), 1 row(s), 0 overlay row(s), 0 paper-denominator row(s).

The one stored row is `results/deterministic_one_case.jsonl`, a
`deterministic-one-case` non-overlay row with `plot_binding: null`.

## Next Honest Actions

1. Fill the approval packet in `results/full_run_plan.md`.
2. Run a model-backed one-case gate only after approval.
3. Promote subset, evolving, held-out, seed-aggregate, and cross-model evidence
   one denominator at a time.
4. Write overlay JSONL only for metrics that can be compared to paper target
   labels without denominator drift.
