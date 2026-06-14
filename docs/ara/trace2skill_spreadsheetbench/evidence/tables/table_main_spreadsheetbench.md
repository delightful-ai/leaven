# Table `tab:main_v1`: Main SpreadsheetBench and WikiTQ Results

**Source**: `tmp/skill_opt_sources/arx_2603.25158/src/tables/table_main_v1.tex`

**Caption**: Main results shown as deltas (%). Skill Author = model that evolved the skill; Skill User = model at inference. Reference rows remain absolute scores for context. Evolved rows show signed deltas. Deepening is measured against the Human-Written baseline; Creation is measured against the Parametric baseline. Avg equally weights in-distribution SpreadsheetBench (Vrf/Soft/Hard, both model scales) and OOD WikiTQ (both model scales), expressed as delta from the corresponding baseline.

**Values are paper targets, not Leaven reproduced results.**

| Skill Author | Mode | Condition | 122B Vrf | 122B Soft | 122B Hard | 122B WikiTQ | 35B Vrf | 35B Soft | 35B Hard | 35B WikiTQ | Avg |
|--------------|------|-----------|----------|-----------|-----------|-------------|---------|----------|----------|------------|-----|
| Reference | absolute | No Skill | 27.67 | 28.90 | 17.57 | 21.50 | 19.00 | 18.00 | 4.60 | 13.33 | 18.35 |
| Reference | absolute | Human-Written | 48.33 | 36.30 | 17.03 | 74.68 | 9.67 | 13.03 | 3.37 | 9.02 | 31.57 |
| Reference | absolute | Parametric | 26.17 | 36.60 | 17.50 | 23.73 | 20.17 | 13.70 | 3.87 | 20.14 | 20.80 |
| Qwen3.5-122B-A10B | Deepening | +Error | +17.50 | +10.30 | +10.40 | +1.62 | +27.00 | +9.44 | +2.86 | +9.26 | +9.18 |
| Qwen3.5-122B-A10B | Deepening | +Success | -21.83 | -8.57 | +0.04 | -10.35 | +9.16 | +3.57 | +1.56 | +12.09 | -0.90 |
| Qwen3.5-122B-A10B | Deepening | +Combined | +21.50 | +10.87 | +12.50 | +4.56 | +21.16 | +8.84 | +1.80 | +6.64 | +9.19 |
| Qwen3.5-122B-A10B | Creation | +Error | +22.83 | +3.77 | +5.87 | +7.89 | +8.66 | +9.53 | +4.00 | +2.06 | +7.04 |
| Qwen3.5-122B-A10B | Creation | +Success | +15.33 | -0.93 | +4.33 | +23.70 | +12.83 | +11.57 | +6.13 | +30.36 | +17.62 |
| Qwen3.5-122B-A10B | Creation | +Combined | +0.16 | -9.23 | -1.40 | +32.32 | -1.17 | +3.73 | +1.36 | +29.70 | +14.96 |
| Qwen3.5-35B-A3B | Deepening | +Error | +16.67 | +8.50 | +8.14 | -6.36 | +17.33 | +9.17 | +4.83 | +2.71 | +4.47 |
| Qwen3.5-35B-A3B | Deepening | +Success | -22.00 | -8.83 | -0.50 | +1.46 | +11.00 | +3.64 | +0.83 | +43.23 | +9.85 |
| Qwen3.5-35B-A3B | Deepening | +Combined | +6.67 | +3.87 | +4.17 | +2.65 | +20.00 | +5.77 | +2.36 | +42.20 | +14.78 |
| Qwen3.5-35B-A3B | Creation | +Error | +1.00 | -7.70 | +1.03 | +57.65 | +3.83 | +7.30 | +2.66 | +12.66 | +18.26 |
| Qwen3.5-35B-A3B | Creation | +Success | +5.33 | -4.57 | +2.43 | +9.09 | +5.66 | +5.80 | +2.63 | +3.31 | +4.54 |
| Qwen3.5-35B-A3B | Creation | +Combined | -0.84 | -9.17 | -1.63 | +30.82 | -0.17 | +4.40 | +1.26 | +18.00 | +11.69 |

## Raw Source Fidelity Note

This Markdown table normalizes the LaTeX source table into one row per displayed condition while preserving exact numeric cell text. Cell colors and bold emphasis from the source table are presentation metadata and are not reproduced here.
