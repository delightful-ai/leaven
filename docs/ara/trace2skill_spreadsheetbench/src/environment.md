# Environment

## Paper / Upstream Environment

| Item | Value | Source |
|------|-------|--------|
| Upstream repository | `https://github.com/Qwen-Applications/Trace2Skill` | `tmp/repros/trace2skill-upstream/README.md` |
| Local upstream checkout | `tmp/repros/trace2skill-upstream` | `docs/working-memory/trace2skill-replication.md` |
| Vendor checkout | `/Users/darin/vendor/github.com/Qwen-Applications/Trace2Skill` | `docs/working-memory/trace2skill-replication.md` |
| Python dependencies named by upstream | `openai`, `tqdm`, `openpyxl`, `requests`, `diskcache` | `tmp/repros/trace2skill-upstream/README.md` |
| API style | OpenAI-compatible chat APIs | `tmp/repros/trace2skill-upstream/README.md` |
| Spreadsheet data path | `data/spreadsheetbench_verified/spreadsheetbench_verified_400` | `tmp/repros/trace2skill-upstream/README.md` |
| Hardware for runtime comparison | 8-GPU A800 node | `tmp/skill_opt_sources/arx_2603.25158/full_source.md:599` |

## Leaven Environment

| Item | Value | Source |
|------|-------|--------|
| Mechanics crate | `examples/trace2skill_spreadsheetbench` | `examples/AGENTS.md` |
| Tiny live proxy | `examples/trace2skill_tiny_live` | `examples/trace2skill_tiny_live/AGENTS.md` |
| Goal handoff | `docs/working-memory/trace2skill-ara-reproduction-goal-handoff.yaml` | local handoff |
| Execution plan | `docs/plans/2026-06-14-trace2skill-ara-reproduction.md` | local plan |

## Approval Boundary

Do not launch Qwen/vLLM-scale runs until model availability, hardware, workers,
merge batch size, ReAct turn budget, seeds, cost, and output artifact plan are
recorded and approved.
