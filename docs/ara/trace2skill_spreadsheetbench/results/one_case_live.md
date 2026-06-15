# One-Case ACP Worker Result

## Classification

| Field | Value |
|-------|-------|
| Proof classification | `deterministic-one-case` |
| Case | `13-1` |
| Dataset slice | one real `SpreadsheetBench`-Verified case |
| Denominator | `one-case-13-1-only` |
| Solver identity | `local-openpyxl-trace2skill-agent` |
| Serving backend | local deterministic Python worker over Leaven ACP stdio |
| Cost | `0` USD; `1` local agent call in ACP receipt |
| Paper-denominator status | Not a paper reproduction |

This result proves that the Leaven public seam can dispatch one real
SpreadsheetBench case to an external worker, require a workbook artifact, score
that workbook, and write trajectory evidence. It does not prove Qwen3.5,
vLLM, Trace2Skill skill evolution, analyst fan-out execution, hierarchical
merge, held-out rows `200..400`, seed aggregation, cross-model transfer, or
paper metric parity.

## Commands

Prepare the staged run directory:

```bash
cargo run -p trace2skill_spreadsheetbench -- --prepare-one-case-run --run-dir tmp/trace2skill-one-case-live
```

Execute the promoted ACP external-worker path and score the produced workbook:

```bash
cargo run -p trace2skill_spreadsheetbench -- --run-one-case-acp-worker --run-dir tmp/trace2skill-one-case-live --model-id local-openpyxl-trace2skill-agent
```

## Result

| Metric | Value | Source |
|--------|-------|--------|
| Score | `1.0` | `tmp/trace2skill-one-case-live/score_report.json` |
| Matched cells | `120` | `tmp/trace2skill-one-case-live/score_report.json` |
| Total cells | `120` | `tmp/trace2skill-one-case-live/score_report.json` |
| Passed | `true` | `tmp/trace2skill-one-case-live/score_report.json` |
| Output workbook bytes | `8423` | `tmp/trace2skill-one-case-live/13-1_output.xlsx` |
| Output workbook SHA-256 | `131cf073e40f73b5f152d3a4d718532ee6c980e467e48e1a136e1275cd31bf40` | `tmp/trace2skill-one-case-live/13-1_output.xlsx` |

## Artifacts

| Artifact | Path | Purpose |
|----------|------|---------|
| Prepared prompt | `tmp/trace2skill-one-case-live/agent_prompt.md` | Prompt handed to the one-case worker. |
| Init workbook | `tmp/trace2skill-one-case-live/1_13-1_init.xlsx` | Real unsolved benchmark input. |
| Output workbook | `tmp/trace2skill-one-case-live/13-1_output.xlsx` | Worker-produced workbook required for success. |
| ACP result | `tmp/trace2skill-one-case-live/acp_result.json` | `leaven/agent.run` result binding the workbook artifact. |
| Transcript | `tmp/trace2skill-one-case-live/agent_transcript.md` | Worker action trace. |
| Score report | `tmp/trace2skill-one-case-live/score_report.json` | Exact answer-range scorer output. |
| Trajectory evidence | `tmp/trace2skill-one-case-live/trajectory.json` | Leaven trajectory evidence for the scored run. |
| Manifest | `tmp/trace2skill-one-case-live/manifest.json` | Updated run manifest with scored status. |
| Worker script | `tmp/trace2skill-one-case-live/trace2skill_acp_worker.py` | Durable local Python worker used by the ACP command. |
| Result JSONL | `docs/ara/trace2skill_spreadsheetbench/results/deterministic_one_case.jsonl` | Denominator-labeled Leaven result record with `plot_binding: null`. |

## Boundary

This result is written as `results/deterministic_one_case.jsonl`, but its
`plot_binding` is intentionally `null`. A one-case local deterministic solver
score would be misleading if drawn on the same axes as full paper
SpreadsheetBench/Qwen/vLLM targets. It should become a visible plot only if the
ARA grows a dedicated one-case panel with its own denominator label.
