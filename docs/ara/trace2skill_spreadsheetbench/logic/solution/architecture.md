# Architecture

## Component Graph

| Component | Purpose | Inputs | Outputs | Key Interactions |
|-----------|---------|--------|---------|------------------|
| Initial skill `S0` | Starting skill directory for deepening or creation. | Human-Written `xlsx` or Parametric `xlsx-basic`. | Frozen skill for trajectories and analysts. | Must remain frozen during Stage 2 analyst fan-out. |
| Trajectory generator | Runs the ReAct spreadsheet agent over the evolving set. | `S0`, tasks, model, generation config. | Labeled trajectory corpus `T-` and `T+`. | Feeds success/error analysis. |
| Error analyst `A-` | Diagnoses failed trajectories with artifact access. | One failed trajectory, frozen `S0`, files, ground truth. | Causally grounded skill patch or exclusion. | Supports agentic-analysis claim. |
| Success analyst `A+` | Extracts useful behavior patterns from successful trajectories. | One successful trajectory, frozen `S0`. | Skill patch. | Supports combined success/error conditions. |
| Patch merge operator `M` | Consolidates patches hierarchically. | Patch pool, frozen `S0`, merge prompts. | Final patch `p*`. | Applies prevalence bias and conflict prevention. |
| Skill patch application | Applies the final patch programmatically. | `p*`, skill directory. | Evolved skill `S*`. | Must reject non-existent files and conflicting edits. |
| Evaluation runner | Runs baseline/evolved skills on validation/test/OOD sets. | Skill directory, model, dataset slice, seed. | Metrics and logs. | Produces target/reproduction values. |
| Leaven ARA package | Records denominator and evidence. | Paper source, upstream code, Leaven dossiers, result records. | Claims, tables, plots, validation report. | Prevents target plots and mechanics tests from being called reproduction. |

## Reproduction Boundary

Leaven currently owns mechanics through `examples/trace2skill_spreadsheetbench`
and paper-specific proxy live behavior through `examples/trace2skill_tiny_live`.
Full reproduction is only the run path that matches the paper's model, split,
seed, worker, merge, turn-budget, and held-out metrics.
