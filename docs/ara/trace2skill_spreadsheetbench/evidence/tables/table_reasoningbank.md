# Table `tab:reasoning_bank`: Trace2Skill vs ReasoningBank

**Source**: `tmp/skill_opt_sources/arx_2603.25158/src/tables/table_reasoning_bank.tex`

**Caption**: Trace2Skill (Human Written+Combined) vs. ReasoningBank on SpreadsheetBench (same-model Deepening, %). ReasoningBank retrieves success and failure memories at inference via Qwen3-Embedding-8B; +Combined distills the same trajectory pool into a single portable skill with no retrieval module. Bold = best per column.

**Values are paper targets, not Leaven reproduced results.**

| Condition | 122B Vrf | 122B Soft | 122B Hard | 35B Vrf | 35B Soft | 35B Hard |
|-----------|----------|-----------|-----------|---------|----------|----------|
| ReasoningBank | 56.00 | 40.10 | 21.30 | 20.50 | 17.30 | 4.97 |
| Human-Written+Combined (ours) | 69.83 | 47.17 | 29.53 | 29.67 | 18.80 | 5.73 |

## Raw Source Fidelity Note

The Markdown table preserves exact numeric cells from the LaTeX source. The citation key and bold emphasis are presentation metadata.
