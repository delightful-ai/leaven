# Leaven Mechanics Test Evidence

**Source**: `examples/trace2skill_spreadsheetbench/tests/`

**Status**: Leaven mechanics and one-case deterministic evidence only. These
tests do not prove live Trace2Skill analyst calls, live hierarchical merge,
held-out `200..400` execution, seed aggregation, cross-model transfer, or full
Qwen/vLLM paper parity.

| Test target | Claim shape | Proof classification | What it proves | What it does not prove | ARA claims |
|-------------|-------------|----------------------|----------------|------------------------|------------|
| `cargo test -p trace2skill_spreadsheetbench --test manifest` | Scenario | mechanics-smoke | The upstream 400-row SpreadsheetBench-Verified manifest lowers into 400 Leaven cases with row-stable ids and paper train/test split boundaries `0..200` and `200..400`. | Does not execute SpreadsheetBench, run a model, evolve a skill, or score held-out rows. | C06, C07 |
| `cargo test -p trace2skill_spreadsheetbench --test run_artifacts` | Scenario | mechanics-smoke | Upstream-shaped `results.json`, logs, analysis records, and Stage 2 pending analyst fan-out prompts can lower into Leaven evidence while embedding upstream prompt template material. | Does not execute the model-backed Stage 2 analysts or parse live analyst responses. | C06, C07 |
| `cargo test -p trace2skill_spreadsheetbench --test patch_bridge` | Scenario | mechanics-smoke | Trace2Skill JSON patch payloads lower into validated `SkillPatchPlan` values plus concrete `SkillBankChange` applications, including reference-link guardrails. | Does not prove paper patch quality, analyst behavior, or final metric improvement. | C06, C07 |
| `cargo test -p trace2skill_spreadsheetbench --test patch_replay` | Scenario | mechanics-smoke | Saved/live-shaped Stage 2/3 JSON patch artifacts and upstream `--save-intermediates` directories replay through Leaven merge/application primitives, including parse-failure and fan-out status handling. | Does not schedule or execute live analyst/merge model calls, and saved intermediates cannot by themselves prove the paper run was reproduced. | C06, C07 |
| `cargo test -p trace2skill_spreadsheetbench --test one_case` | Example | one-case no-spend preflight | Exact case `13-1` metadata, prompt fragments, workbooks, upstream system prompt, released skill, and deterministic output path can be inspected/rendered. | Does not solve the workbook or score a model output. | C06, C07 |
| `cargo test -p trace2skill_spreadsheetbench --test cli` | Scenario | one-case CLI mechanics | CLI entrypoints inspect/render/compare/prepare/score exact case `13-1` and prepare pending analyst fan-out JSON. | Does not prove live spreadsheet-agent execution, live analyst calls, or paper metrics. | C06, C07 |
| `cargo test -p trace2skill_spreadsheetbench --test workbook_score` | Example | one-case scorer mechanics | The exact `LISTS!A3:D32` answer range scorer distinguishes the golden workbook from the unsolved init workbook for case `13-1`. | Does not prove a real agent can produce the golden workbook. | C06, C07 |
| `cargo test -p trace2skill_spreadsheetbench --test acp_external_worker` | Scenario | deterministic one-case seam proof | A local Python ACP worker can solve real case `13-1`, bind the output workbook artifact to the ACP result, improve the scorer to pass, and the negative test rejects envelope-only success without the workbook. | Does not use Qwen3.5, vLLM, Trace2Skill skill evolution, analyst fan-out, hierarchical merge, held-out split, or seed aggregation. | C06, C07 |

## Current Verification

The latest focused verification result is recorded in
`docs/ara/trace2skill_spreadsheetbench/validation.md`.
