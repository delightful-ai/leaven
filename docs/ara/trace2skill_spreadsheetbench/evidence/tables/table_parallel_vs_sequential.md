# Table `tab:seq_parallel`: Parallel Consolidation vs Sequential Editing

**Source**: `tmp/skill_opt_sources/arx_2603.25158/src/tables/table_seq_parallel.tex`

**Caption**: Parallel consolidation vs. sequential editing on SpreadsheetBench (same-model Deepening, +Error only, %). Seq-B: skill updated after every batch of B trajectories. Bold = best per column.

**Values are paper targets, not Leaven reproduced results.**

| Condition | 122B Vrf | 122B Soft | 122B Hard | 35B Vrf | 35B Soft | 35B Hard | Time |
|-----------|----------|-----------|-----------|---------|----------|----------|------|
| Seq-B=4 | 59.00 | 40.63 | 20.63 | 26.17 | 22.37 | 7.47 | ~15 min |
| Seq-B=1 | 61.83 | 44.40 | 25.40 | 26.00 | 23.83 | 10.57 | ~60 min |
| Parallel (ours) | 65.83 | 46.60 | 27.43 | 27.00 | 22.20 | 8.20 | ~3 min |

## Raw Source Fidelity Note

The Markdown table preserves exact numeric cells and approximate runtime text from the LaTeX source. Bold emphasis is presentation metadata.
