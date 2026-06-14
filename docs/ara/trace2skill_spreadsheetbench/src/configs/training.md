# Training and Evaluation Configuration

## SpreadsheetBench-Verified

| Field | Value | Rationale | Search range | Sensitivity | Source |
|-------|-------|-----------|--------------|-------------|--------|
| Dataset size | 400 samples | Official paper target for SpreadsheetBench-Verified. | Not specified in paper. | Changing row count changes denominator. | `tmp/skill_opt_sources/arx_2603.25158/full_source.md:149` |
| Evolving split | rows `0..200` | Used for trajectory collection and training-set validation. | Not specified in paper. | Mixing held-out rows invalidates test claim. | `tmp/repros/trace2skill-upstream/README.md` |
| Held-out split | rows `200..400` | Final held-out SpreadsheetBench evaluation. | Not specified in paper. | Using any smaller slice is subset proof only. | `tmp/repros/trace2skill-upstream/README.md` |
| Seeds | `41`, `42`, `43` | Paper reports averaging over three random seeds. | Not specified in paper. | Single-seed results are not paper aggregate. | `tmp/skill_opt_sources/arx_2603.25158/full_source.md:149` |
| Stage 1 trajectories | 1 trajectory per problem | Paper implementation detail. | Not specified in paper. | More/fewer trajectories change patch pool. | `tmp/skill_opt_sources/arx_2603.25158/full_source.md:159` |
| Stage 2 workers | 128 sub-agents | Paper implementation detail and runtime claim basis. | Not specified in paper. | Lower worker counts change runtime denominator. | `tmp/skill_opt_sources/arx_2603.25158/full_source.md:159` |
| Merge batch size | 32 | Paper implementation detail. | Not specified in paper. | Changes merge tree shape. | `tmp/skill_opt_sources/arx_2603.25158/full_source.md:159` |
| ReAct turn budget | 100 | Paper implementation detail. | Not specified in paper. | Changes task-solving and analyst capacity. | `tmp/skill_opt_sources/arx_2603.25158/full_source.md:159` |

## Non-Spreadsheet Targets

| Dataset | Value | Rationale | Source |
|---------|-------|-----------|--------|
| DAPO-Math-Test-100 | pass rate target table | Math generalization target. | `tmp/skill_opt_sources/arx_2603.25158/src/tables/table_math.tex` |
| AIME 2026 | avg@8 over 30 problems | Math OOD target. | `tmp/skill_opt_sources/arx_2603.25158/src/tables/table_math.tex` |
| DocVQA | 5,349 validation split; first 2,700 evolving and remaining 2,649 held out | VQA target. | `tmp/skill_opt_sources/arx_2603.25158/full_source.md:479` |
