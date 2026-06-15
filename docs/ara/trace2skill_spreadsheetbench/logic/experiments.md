# Experiments

## E01: Main SpreadsheetBench and WikiTQ Paper Target Verification
- **Verifies**: C01
- **Setup**:
  - Model: Qwen3.5-122B-A10B and Qwen3.5-35B-A3B.
  - Hardware: vLLM serving; exact hardware for reproduction requires approval.
  - Dataset: SpreadsheetBench-Verified and WikiTQ converted to spreadsheet format.
  - System: Trace2Skill deepening and creation modes.
- **Procedure**:
  1. Transcribe Table `tab:main_v1` as paper targets.
  2. Re-run the paper protocol only after model/hardware approval.
  3. Overlay Leaven results without modifying paper target values.
- **Metrics**: Vrf, Soft, Hard, WikiTQ, Avg.
- **Expected outcome**:
  - Evolved skills outperform their stated baselines in the pattern reported by the paper.
- **Baselines**: No Skill, Human-Written, Parametric.
- **Dependencies**: E09

## E02: Parallel Consolidation Versus Sequential Editing
- **Verifies**: C02
- **Setup**:
  - Model: same-model Deepening with +Error only.
  - Hardware: 8-GPU A800 node in paper; Leaven hardware requires approval.
  - Dataset: SpreadsheetBench.
  - System: Seq-B=4, Seq-B=1, Parallel.
- **Procedure**:
  1. Preserve Table `tab:seq_parallel` as the target.
  2. Re-run sequential and parallel conditions only under approved compute.
  3. Compare both score and runtime shape.
- **Metrics**: Vrf, Soft, Hard, runtime.
- **Expected outcome**:
  - Parallel consolidation is faster and competitive or better on the primary score.
- **Baselines**: Seq-B=4, Seq-B=1.
- **Dependencies**: E09

## E03: Portable Skill Versus ReasoningBank Retrieval
- **Verifies**: C03
- **Setup**:
  - Model: same-model Deepening.
  - Hardware: not specified in paper table.
  - Dataset: SpreadsheetBench.
  - System: ReasoningBank retrieval and Trace2Skill Human-Written+Combined.
- **Procedure**:
  1. Preserve Table `tab:reasoning_bank` as the target.
  2. Recreate retrieval and skill-consumption runs under approved compute.
  3. Compare each SpreadsheetBench cell.
- **Metrics**: Vrf, Soft, Hard for 122B and 35B skill users.
- **Expected outcome**:
  - A distilled portable skill outperforms retrieval memory under the paper protocol.
- **Baselines**: ReasoningBank.
- **Dependencies**: E09

## E04: Agentic Error Analysis Ablation
- **Verifies**: C04
- **Setup**:
  - Model: Qwen3.5-122B-A10B and Qwen3.5-35B-A3B.
  - Hardware: not specified in paper table.
  - Dataset: SpreadsheetBench and WikiTQ.
  - System: +Error agentic analysis versus +Error LLM.
- **Procedure**:
  1. Preserve Table `tab:agentic_ablation` as the target.
  2. Re-run or import faithful upstream artifacts for both analyst modes.
  3. Compare Avg and per-slice transfer.
- **Metrics**: Vrf, Soft, Hard, WikiTQ, Avg.
- **Expected outcome**:
  - Agentic error analysis is more transferable than single-call analysis.
- **Baselines**: +Error LLM.
- **Dependencies**: E09

## E05: Math Skill Transfer Target
- **Verifies**: C05
- **Setup**:
  - Model: Qwen3.5-122B-A10B and Qwen3.5-35B-A3B.
  - Hardware: not specified in paper table.
  - Dataset: DAPO-Math-Test-100 and AIME 2026.
  - System: error-derived skills from scratch.
- **Procedure**:
  1. Preserve Table `tab:math` as paper target.
  2. Keep this out of the SpreadsheetBench reproduction claim unless explicitly run.
- **Metrics**: D-Test pass rate and AIME avg@8.
- **Expected outcome**:
  - Evolved math skills improve over No Skill.
- **Baselines**: No Skill.
- **Dependencies**: none

## E06: DocVQA Skill Transfer Target
- **Verifies**: C05
- **Setup**:
  - Model: Qwen3.5-122B-A10B and Qwen3.5-35B-A3B.
  - Hardware: not specified in paper table.
  - Dataset: DocVQA validation split with first segment as evolving set and remaining segment as held-out evaluation set.
  - System: error-derived skills from scratch.
- **Procedure**:
  1. Preserve Table `tab:vqa` as paper target.
  2. Keep this out of the SpreadsheetBench reproduction claim unless explicitly run.
- **Metrics**: ANLS and Accuracy.
- **Expected outcome**:
  - Skill benefits vary by author/user model combination.
- **Baselines**: No Skill.
- **Dependencies**: none

## E07: Current Leaven Mechanics Classification
- **Verifies**: C06
- **Setup**:
  - Model: deterministic local mechanics unless explicitly live-gated.
  - Hardware: local development machine.
  - Dataset: local upstream SpreadsheetBench artifacts and exact case `13-1`.
  - System: `examples/trace2skill_spreadsheetbench`.
- **Procedure**:
  1. Run focused mechanics tests.
  2. Map each test to this ARA as mechanics, one-case, subset, held-out, or full-paper evidence.
  3. Refuse to promote mechanics to full reproduction.
- **Metrics**: test result, proof classification, artifact paths.
- **Expected outcome**:
  - Current Leaven assets remain mechanics-smoke or one-case proof until live/full runs exist.
- **Baselines**: none.
- **Dependencies**: none
- **Current evidence**: `evidence/leaven_mechanics_tests.md`

## E08: Reproduction Denominator Overlay
- **Verifies**: C07
- **Setup**:
  - Model: any Leaven-run model must be recorded.
  - Hardware: any Leaven-run hardware must be recorded.
  - Dataset: exact slice must be recorded.
  - System: ARA plotting plus Leaven result JSONL overlay.
- **Procedure**:
  1. Generate target plots from ARA evidence.
  2. Add Leaven result records separately.
  3. Produce overlays and closeout by denominator.
- **Metrics**: denominator label, paper target value, Leaven result value, source command, artifacts.
- **Expected outcome**:
  - Closeout language matches the proven denominator.
- **Baselines**: paper target rows.
- **Dependencies**: E01, E02, E03, E04, E09

## E09: Full Paper-Denominator Approval Gate
- **Verifies**: C07
- **Setup**:
  - Model: Qwen3.5-122B-A10B and Qwen3.5-35B-A3B only after availability is confirmed.
  - Hardware: vLLM serving plan with approved GPU/runtime/cost envelope.
  - Dataset: 400-row SpreadsheetBench-Verified with rows `0..200` for evolving and rows `200..400` for held-out evaluation.
  - System: Trace2Skill trajectory generation, success/error analyst fan-out, hierarchical merge, and final skill evaluation.
- **Procedure**:
  1. Fill and approve `results/full_run_plan.md`.
  2. Run subset gates in order: one case, small `N`, rows `0..200`, rows `200..400`, seeds `41/42/43`, then cross-model/condition rows.
  3. Stop on any denominator drift or missing artifact.
- **Metrics**: approval packet completeness, model identity, split, seeds, worker count, merge batch size, ReAct turn budget, cost/runtime, artifact root.
- **Expected outcome**:
  - The user can approve or reject the exact compute/spend plan before any Qwen/vLLM-scale execution.
- **Baselines**: none.
- **Dependencies**: E07, E08
