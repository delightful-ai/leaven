## Trace2Skill Tiny Loop

Status: active paper-specific example surface.

This directory is a small live Codex harness for the Trace2Skill core loop. It
exists to preserve the paper's causal execution shape before extracting any
shared Leaven primitive.

Paper anchors:

- Trace2Skill runs three stages: trajectory generation, parallel analyst patch
  proposal, and conflict-free consolidation:
  `tmp/skill_opt_sources/arx_2603.25158/full_source.md:19`.
- The target skill is preloaded for trajectory collection:
  `tmp/skill_opt_sources/arx_2603.25158/full_source.md:896`.
- Error analysts inspect traces, files, and ground truth before proposing a
  causally grounded patch:
  `tmp/skill_opt_sources/arx_2603.25158/full_source.md:107`.
- Analysts operate independently over a frozen copy of the initial skill:
  `tmp/skill_opt_sources/arx_2603.25158/full_source.md:111`.
- The merge coordinator deduplicates, resolves conflicts, preserves unique
  insights, and enforces line-level independence:
  `tmp/skill_opt_sources/arx_2603.25158/full_source.md:984`.
- The consolidated patch is applied programmatically:
  `tmp/skill_opt_sources/arx_2603.25158/full_source.md:1045`.

The live command runs two tiny CSV editing trajectories:

1. one failure trajectory where the frozen skill over-applies row deletion
   outside a target range;
2. one success trajectory where the same frozen skill works on a full-file
   deletion task;
3. independent error and success analysts propose patches;
4. a coordinator consolidates those patches into one skill update;
5. guardrails validate file targets and operation independence;
6. the updated skill is replayed on the failed task and evaluated.

Known deviations:

- Codex/GPT-5.4-mini replaces the Qwen3.5 models from the paper.
- CSV files replace SpreadsheetBench `xlsx` files while preserving the
  spreadsheet-editing failure mode.
- The tiny proof uses serial analyst calls; analyst independence is preserved,
  but real parallelism and 128 workers are deferred.
- The merge tree has one merge level rather than many hierarchical levels.
- Full SpreadsheetBench/WikiTQ/DAPO/DocVQA scale, cross-model transfer, and
  score tables remain deferred.

Commands:

```bash
bash examples/trace2skill_tiny_live/scripts/run_tiny_live.sh --preflight
LEAVEN_CODEX_LIVE=1 bash examples/trace2skill_tiny_live/scripts/run_tiny_live.sh --live
```

