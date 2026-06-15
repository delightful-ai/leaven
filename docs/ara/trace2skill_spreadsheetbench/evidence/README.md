# Evidence Index

All numeric values in this evidence layer are paper targets unless an entry is
explicitly labeled as a Leaven result. No Leaven result overlays exist yet.

Check this index with:

```bash
uv run --with pyyaml python scripts/check_trace2skill_evidence_bindings.py docs/ara/trace2skill_spreadsheetbench
```

| Evidence file | Source | Claims | Notes |
|---------------|--------|--------|-------|
| [tables/table_main_spreadsheetbench.md](tables/table_main_spreadsheetbench.md) | `tmp/skill_opt_sources/arx_2603.25158/src/tables/table_main_v1.tex` | C01 | Main SpreadsheetBench/WikiTQ paper target table. |
| [tables/table_parallel_vs_sequential.md](tables/table_parallel_vs_sequential.md) | `tmp/skill_opt_sources/arx_2603.25158/src/tables/table_seq_parallel.tex` | C02 | Parallel versus sequential target table. |
| [tables/table_reasoningbank.md](tables/table_reasoningbank.md) | `tmp/skill_opt_sources/arx_2603.25158/src/tables/table_reasoning_bank.tex` | C03 | Retrieval-memory baseline target table. |
| [tables/table_agentic_ablation.md](tables/table_agentic_ablation.md) | `tmp/skill_opt_sources/arx_2603.25158/src/tables/table_agentic_ablation.tex` | C04 | Agentic versus LLM-only analysis target table. |
| [tables/table_math.md](tables/table_math.md) | `tmp/skill_opt_sources/arx_2603.25158/src/tables/table_math.tex` | C05 | Math transfer target table. |
| [tables/table_vqa.md](tables/table_vqa.md) | `tmp/skill_opt_sources/arx_2603.25158/src/tables/table_vqa.tex` | C05 | DocVQA transfer target table. |
| [figures/figure_trace2skill_framework.md](figures/figure_trace2skill_framework.md) | `tmp/skill_opt_sources/arx_2603.25158/src/figures/trace2skill_framwork.png` | C01, C02, C04 | Source framework figure and caption. |
| [leaven_mechanics_tests.md](leaven_mechanics_tests.md) | `examples/trace2skill_spreadsheetbench/tests/` | C06, C07 | Focused Leaven mechanics and one-case proof classification. |
| [prompt_templates.md](prompt_templates.md) | `tmp/repros/trace2skill-upstream/skill_evolver/prompts/` | C01, C02, C03, C04, C07 | Prompt-template family index for agent, analyst, merge, verification, and released-skill prompts. |

## Missing Evidence

| Evidence | Status |
|----------|--------|
| Leaven result overlay JSONL | Not yet generated. |
| One-case `13-1` ACP worker result | `results/one_case_live.md` records a deterministic local ACP worker proof. This is not model-backed paper parity. |
| Full held-out `200..400` Leaven run | Not yet generated. |
| Seed aggregate `41/42/43` Leaven run | Not yet generated. |
