# Full Paper-Denominator Run Plan

This document is an approval gate. It is not permission to launch a full run.

The Trace2Skill / SpreadsheetBench paper denominator is not reproduced until
Leaven runs the approved model, serving, split, seed, worker, merge, and turn
budget conditions and writes denominator-labeled result records.

## Required Approval Inputs

| Requirement | Paper target | Leaven status | Approval needed |
|-------------|--------------|---------------|-----------------|
| 122B model | `Qwen3.5-122B-A10B` | Public model/source availability researched; not provisioned locally. | Model source, weights/API endpoint, license, and operator approval. |
| 35B model | `Qwen3.5-35B-A3B` | Public model/source availability researched; not provisioned locally. | Model source, weights/API endpoint, license, and operator approval. |
| Serving backend | vLLM | Not provisioned for this run. | Host/GPU plan, vLLM version, tensor-parallel shape, request limits. |
| Dataset | 400-row SpreadsheetBench-Verified path | Local upstream sample paths exist; full run path must be rechecked. | Exact `data/spreadsheetbench_verified/spreadsheetbench_verified_400` path and checksum/provenance. |
| Evolving split | rows `0..200` | Captured in ARA config. | Approval that these rows feed trajectory collection and skill evolution only. |
| Held-out split | rows `200..400` | Captured in ARA config. | Approval that these rows are untouched until final evaluation. |
| Seeds | `41`, `42`, `43` | Captured in ARA config. | Approval for all three seeds; single seed remains subset evidence. |
| Stage 2 workers | `128` | Captured in ARA config. | Approval for local/concurrent worker limit and failure policy. |
| Merge batch size | `32` | Captured in ARA config. | Approval for merge tree shape and retry policy. |
| ReAct turn budget | `100` | Captured in ARA config. | Approval for per-case agent budget and timeout behavior. |
| Cost/runtime envelope | Paper reports runtime, not Leaven cost. | Unknown. | Explicit max USD, max wall time, max GPU hours, storage path, and stop condition. |
| Reproduction tolerance | Proposed in `src/configs/tolerance.md` | Not approved. | Approve per-metric tolerance, runtime interpretation, retry, and failure-accounting policy. |
| Credentials | OpenAI-compatible endpoint style in upstream. | Not configured for this plan. | Token/env var names, redaction policy, and log retention policy. |
| Artifact root | Leaven run artifacts | Proposed: `tmp/trace2skill-paper-denominator/<run_id>` until promoted. | Approval for artifact retention and whether outputs can be committed, archived, or externalized. |

## Subset Gates

| Gate | Scope | Required evidence | Allowed label | Forbidden label |
|------|-------|-------------------|---------------|-----------------|
| G1 | case `13-1` | run manifest, output workbook, ACP/result envelope, transcript, score report, trajectory | `deterministic-one-case` or `model-one-case` | paper reproduction |
| G2 | small `N` cases | per-case manifests, trajectories, score reports, analyst fan-out sanity | `paper-subset` | held-out split reproduced |
| G3 | rows `0..200` | trajectory generation, patch pool, merge tree, evolved skill, training/evolving validation | `evolving-split-run` | held-out result |
| G4 | rows `200..400` | untouched held-out score for one approved seed/model condition | `held-out-single-seed-candidate` | paper aggregate |
| G5 | seeds `41/42/43` | three held-out runs for one model/condition with identical protocol | `seed-aggregate-candidate` | cross-model paper reproduction |
| G6 | 122B and 35B target conditions | all paper-required model/condition rows with target overlays and artifact audit | `paper-denominator-reproduction` | anything stronger than the actual completed rows |

## Execution Order

1. Confirm model and vLLM availability for `Qwen3.5-122B-A10B` and `Qwen3.5-35B-A3B`.
2. Confirm dataset path, row order, and split materialization.
3. Run a model-backed one-case gate and write a result note distinct from the deterministic local ACP worker proof.
4. Run a small `N`-case subset with trajectory import and analyst fan-out sanity checks.
5. Run rows `0..200` for trajectory collection and skill evolution.
6. Run rows `200..400` for held-out evaluation only after the evolved skill is fixed.
7. Repeat for seeds `41`, `42`, and `43`.
8. Repeat approved model/condition rows needed for the target table being claimed.
9. Write `results/*.jsonl` only for metrics that bind to paper target plot labels without denominator drift.
10. Regenerate plots and run the ARA validator.

## Stop Conditions

- Stop if model identity, serving backend, generation config, split, seed, worker count, merge batch size, or turn budget differs without an explicit deviation record.
- Stop if any output workbook is missing but an envelope claims success.
- Stop if held-out rows are touched during evolving/training.
- Stop if cost or runtime exceeds the approved envelope.
- Stop if result rows cannot name source command and artifact paths.

## Approval Packet To Collect Before Running

```yaml
models:
  qwen_122b: null
  qwen_35b: null
serving:
  backend: vLLM
  host: null
  version: null
  tensor_parallel: null
  gpu_type: null
  gpu_count: null
dataset:
  path: data/spreadsheetbench_verified/spreadsheetbench_verified_400
  checksum_or_manifest: null
protocol:
  seeds: [41, 42, 43]
  stage2_workers: 128
  merge_batch_size: 32
  react_turn_budget: 100
budget:
  max_usd: null
  max_wall_clock_hours: null
  max_gpu_hours: null
credentials:
  api_key_env: null
  redaction_policy: null
  log_retention: null
tolerance:
  policy: docs/ara/trace2skill_spreadsheetbench/src/configs/tolerance.md
  approved: null
artifacts:
  root: tmp/trace2skill-paper-denominator/<run_id>
  retention: null
  expected:
    - run_metadata.json
    - dataset_manifest.json
    - trajectory_generation/{seed}/{case_id}/manifest.json
    - trajectory_generation/{seed}/{case_id}/trajectory.json
    - trajectory_generation/{seed}/{case_id}/score_report.json
    - skill_evolution/{seed}/patch_pool.jsonl
    - skill_evolution/{seed}/merge_tree.json
    - skill_evolution/{seed}/skill/SKILL.md
    - heldout_eval/{seed}/{case_id}/manifest.json
    - heldout_eval/{seed}/{case_id}/trajectory.json
    - heldout_eval/{seed}/{case_id}/score_report.json
    - leaven_results.jsonl
approval:
  approved_by: null
  approved_at: null
```

All `null` values must be resolved before full-denominator execution.
The `artifacts.root` template must also be replaced with a concrete run id.

Before a Qwen/vLLM-scale run, check the packet:

```bash
uv run --with pyyaml python scripts/check_trace2skill_approval_packet.py docs/ara/trace2skill_spreadsheetbench
```

The command must fail while the packet is unresolved. To verify that the current
blocked state is intentional, run:

```bash
uv run --with pyyaml python scripts/check_trace2skill_approval_packet.py docs/ara/trace2skill_spreadsheetbench --expect-blocked
```

## Availability Research

`model_availability.md` confirms that public Hugging Face repositories exist for
both paper model names and that vLLM/OpenAI-compatible serving paths are
documented. This does not approve execution. The approval packet above still
needs concrete endpoint, hardware, credentials, cost, retention, and tolerance
approval values.
