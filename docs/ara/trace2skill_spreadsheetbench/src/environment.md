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

## Full Run Approval Matrix

| Item | Required value | Status |
|------|----------------|--------|
| 122B model | `Qwen3.5-122B-A10B` | Pending approval. |
| 35B model | `Qwen3.5-35B-A3B` | Pending approval. |
| Serving backend | vLLM | Pending host/version/tensor-parallel plan. |
| Dataset | `data/spreadsheetbench_verified/spreadsheetbench_verified_400` | Manifested in `results/dataset_manifest.json`: 400 records, split `0..200` / `200..400`, no missing spreadsheet directories. |
| Seeds | `41`, `42`, `43` | Pending approval for all three. |
| Stage 2 workers | `128` | Pending concurrency and failure-policy approval. |
| Merge batch size | `32` | Pending merge-tree approval. |
| ReAct turn budget | `100` | Pending timeout/turn-budget approval. |
| Cost envelope | explicit max USD/GPU-hours/wall time | Pending. |
| Reproduction tolerance | `src/configs/tolerance.md` | Pending approval. |
| Artifact root | explicit durable path and retention policy | Pending. |

See `results/full_run_plan.md` for the full approval packet and subset gates.
See `results/model_availability.md` for public model/serving availability
research.
