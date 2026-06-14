# Table `tab:agentic_ablation`: Agentic Error Analysis vs Single-Call LLM

**Source**: `tmp/skill_opt_sources/arx_2603.25158/src/tables/table_agentic_ablation.tex`

**Caption**: Agentic error analysis (+Error) vs. single-LLM-call error analysis (+Error LLM) across all Author--Mode combinations (%). Same-model cells are shaded; plain cells are cross-model transfer. Bold = better of the two conditions per cell.

**Values are paper targets, not Leaven reproduced results.**

| Skill Author | Mode | Condition | 122B Vrf | 122B Soft | 122B Hard | 122B WikiTQ | 35B Vrf | 35B Soft | 35B Hard | 35B WikiTQ | Avg |
|--------------|------|-----------|----------|-----------|-----------|-------------|---------|----------|----------|------------|-----|
| Qwen3.5-122B-A10B | Deepening | +Error (ours) | 65.83 | 46.60 | 27.43 | 76.30 | 36.67 | 22.47 | 6.23 | 18.28 | 40.75 |
| Qwen3.5-122B-A10B | Deepening | +Error LLM | 67.00 | 43.93 | 25.23 | 39.81 | 25.00 | 22.43 | 6.23 | 11.24 | 28.58 |
| Qwen3.5-122B-A10B | Creation | +Error (ours) | 49.00 | 40.37 | 23.37 | 31.62 | 28.83 | 23.23 | 7.87 | 22.20 | 27.84 |
| Qwen3.5-122B-A10B | Creation | +Error LLM | 27.17 | 27.73 | 16.20 | 47.26 | 19.83 | 17.60 | 4.70 | 23.30 | 27.08 |
| Qwen3.5-35B-A3B | Deepening | +Error (ours) | 65.00 | 44.80 | 25.17 | 68.32 | 27.00 | 22.20 | 8.20 | 11.73 | 36.04 |
| Qwen3.5-35B-A3B | Deepening | +Error LLM | 37.83 | 22.93 | 12.83 | 77.05 | 30.50 | 20.17 | 8.73 | 9.95 | 32.83 |
| Qwen3.5-35B-A3B | Creation | +Error (ours) | 27.17 | 28.90 | 18.53 | 81.38 | 24.00 | 21.00 | 6.53 | 32.80 | 39.06 |
| Qwen3.5-35B-A3B | Creation | +Error LLM | 22.00 | 27.67 | 16.60 | 54.61 | 23.50 | 16.87 | 4.93 | 11.24 | 25.76 |

## Raw Source Fidelity Note

The Markdown table preserves exact numeric cells from the LaTeX source. Shading and bold emphasis are presentation metadata.
