# Heuristics

## H01: Preserve Analyst Independence
- **Rationale**: The paper states analysts operate on frozen `S0` with no visibility into other patches to avoid premature convergence.
- **Sensitivity**: Sharing patches early would change the paper algorithm and invalidate the parallel consolidation comparison.
- **Bounds**: Applies to Stage 2 patch proposal; Stage 3 deliberately sees multiple patches.
- **Code ref**: `src/execution/trace2skill_pipeline.py`
- **Source**: `tmp/skill_opt_sources/arx_2603.25158/full_source.md:111`

## H02: Treat Recurrence as Systematic Signal
- **Rationale**: The merge prompt gives higher priority to prevalent edits because recurrence across independent patches suggests systematic task properties.
- **Sensitivity**: Too-low prevalence filtering may overfit; too-high filtering may discard rare but important edge cases.
- **Bounds**: Applies during Stage 3 hierarchical consolidation.
- **Code ref**: `src/execution/trace2skill_pipeline.py`
- **Source**: `tmp/skill_opt_sources/arx_2603.25158/full_source.md:129`

## H03: Validate Patch Application Programmatically
- **Rationale**: The paper names guardrails for missing files, overlapping line edits, and skill format validation.
- **Sensitivity**: Accepting fuzzy or partially applied patches can create fake proof of evolution.
- **Bounds**: Applies to final patch application and Leaven replay.
- **Code ref**: `src/execution/trace2skill_pipeline.py`
- **Source**: `tmp/skill_opt_sources/arx_2603.25158/full_source.md:125`

## H04: Validate Evolved Skill Before Held-Out Evaluation
- **Rationale**: Upstream reproduction notes say the paper runs skill evolution plus training-set validation across seeds and selects the evolved skill with best training-set validation before held-out evaluation.
- **Sensitivity**: Skipping validation changes seed selection and can damage robustness due to hallucinated edits.
- **Bounds**: Applies to the full paper-denominator run, not no-spend mechanics.
- **Code ref**: `src/execution/trace2skill_pipeline.py`
- **Source**: `tmp/repros/trace2skill-upstream/README.md`

## H05: Keep Paper Targets Separate From Leaven Results
- **Rationale**: A target plot can look like success while containing only paper numbers.
- **Sensitivity**: Mixing targets and run outputs makes closeout untrustworthy.
- **Bounds**: Applies to every plot and result record generated from this ARA.
- **Code ref**: `src/execution/trace2skill_pipeline.py`
- **Source**: `docs/working-memory/trace2skill-ara-reproduction-goal-handoff.yaml`
