# GEPA Upstream Parity Matrix

Status: active execution matrix.
Date: 2026-05-17.

This is the working proof surface for the GEPA parity goal. It is not product
law. Product law remains in `docs/specs/initial_library.md`,
`docs/specs/gepa_reference_behavior.md`, and
`docs/specs/gepa_aime_paper_parity.md`.

Rules for this matrix:

- Every row needs an upstream anchor, a Leaven path, a proof, and a verdict.
- Verdicts are `proven`, `gap`, `intentional-delta`, or `unknown`.
- A Leaven-better behavior still needs tests and report/docs disclosure.
- Do not use deterministic P8 smoke, fake reflectors, or topology-only checks
  as proof for live GEPA/AIME parity.

## Matrix

| Requirement | Upstream anchor | Leaven path | Current proof | Verdict | Next action |
| --- | --- | --- | --- | --- | --- |
| Core GEPA defaults are explicit: Pareto parent selection, full validation, epoch minibatch 3, round-robin part selection, strict acceptance, skip-perfect, merge off | `gepa_reference_behavior.md` section 3.1; upstream `src/gepa/api.py` | `crates/leaven-gepa/src/builder.rs`, `src/optimizer.rs`, `src/validation.rs` | `gepa_default_validation_policy_is_full_validation`, `public_reference_builder_requires_surface_then_reflector`, `gepa_default_max_iterations_is_not_one_iteration_smoke_config`; fallback selector renamed to `PopulationBestFallback` and removed from `leaven_gepa::prelude` | proven | Keep selector internals out of ordinary GEPA examples; advanced root exports remain ablation/customizer surface. |
| Seed full validation happens before any train minibatch | upstream `src/gepa/core/engine.py::run`; spec section 4/8 | `Gepa::initialize`, `validate_candidate` | `reference_state_seed_validation_initializes_candidate_zero_before_train` | proven | Keep this row covered when changing initialize/checkpoint behavior. |
| Seed validation initializes candidate index 0, lineage, validation subscores, frontier membership, and metric-call counters | upstream `GEPAState.initialize_gepa_state`; spec sections 4 and 11 | `crates/leaven-gepa/src/state.rs`, `src/report.rs` | `reference_state_seed_validation_initializes_candidate_zero_before_train`; `GepaReferenceState` unit coverage | proven | Add report-level assertion for seed lineage/subscores if missing from public path. |
| Reference parent selection samples validation Pareto frontier frequency after dominance pruning | upstream `ParetoCandidateSelector`, `select_program_candidate_from_pareto_front`, `remove_dominated_programs`; spec sections 5 and 19.2 | `GepaReferenceState::select_by_validation_frontier_frequency`, `Gepa::select_reference_parent` | `default_parent_selection_samples_validation_frontier_frequency`; state-level frequency, dominance, and RNG-restore tests | proven | Add next-parent-after-accepted-validation proof when touching frontier admission. |
| Train sampler uses only train/search cases, defaults to minibatch size 3, and resumes deterministically | upstream reflective mutation sampler; spec sections 5 and 19.2 | `EpochShuffled`, checkpoint state | `epoch_shuffled_samples_train_with_seed_and_restores_cursor`, `gepa_default_sampler_uses_train_minibatches_without_validation_or_test_cases` | gap | Add restore-through-optimizer test proving next minibatch after checkpoint matches uninterrupted run. |
| Parent and child screening use the exact same ordered train minibatch | upstream `reflective_mutation.py` child evaluation; spec sections 5 and 19.3 | `run_iteration`, `process_proposal` | proposal-attempt rows carry parent/child cases; `parent_and_child_screen_on_same_ordered_train_cases`; equal-score rejection also asserts parent/child case equality | proven | Keep proposal-attempt case lists in report snapshots. |
| Parent selection precedes train minibatch sampling | upstream reflective mutation preparation; spec phase order section 4 | `run_iteration` event/order path | `parent_and_child_screen_on_same_ordered_train_cases` asserts parent-selected event precedes train-minibatch event | proven | Fold into broader GEPA phase-order assertion row. |
| Acceptance is strict improvement on train minibatch only | upstream `_process_proposal_output`; spec section 5 | `StrictImprovement`, `process_proposal` | `StrictImprovement` unit assertions; `strict_equal_score_child_is_rejected_without_full_validation_or_admission` | proven | Add negative proof for acceptance policy variants only if they become ordinary public profiles. |
| Accepted child is full-validated before admission to GEPA reference state/frontier | upstream `_run_full_eval_and_add`; spec sections 5 and 19.3 | `accept_child`, `validate_candidate`, `GepaReferenceState::add_validated_candidate` | `accepted_child_enters_reference_state_only_after_full_validation` | proven | Extend to prove next parent selection sees updated frontier. |
| Evaluation cache reuses per candidate/case rows across overlapping GEPA requests and cache hits do not increment GEPA metric calls | upstream `EvaluationCache`; spec sections 18 and 19.4 | engine per-case backfill via `evaluate_casewise` | `gepa_reuses_evaluation_cache_per_candidate_case_across_different_requests` | gap | Add full-validation cache-hit and resume-no-repeat seed validation tests. |
| All-perfect and no-reflective-example skips happen before part selection, dataset building, reflector calls, or provider work | upstream reflective mutation skip gates; spec sections 6 and 19.5 | `propose_candidate`, `GepaSkipReason`, `GepaProposalAttempt` | `no_reflective_examples_skip_before_reflector_provider_work`, `all_scores_perfect_skip_before_part_dataset_or_reflector_work` | proven | Keep P8 report mapping in sync with skip reasons. |
| Reflection request/model experience matches upstream for claimed profile | upstream instruction proposal, optimize-anything AIME prompt, DSPy ChatAdapter AIME solver | `DefaultReflectionRenderer`, `PlainTextEditParser`, `examples/p8_aime_gepa` solver/reflection rendering | `cargo test -p p8_aime_gepa reflection_prompt` passed copied-upstream snapshots for default GEPA and optimize-anything AIME full markdown; LM tests cover renderer/parser mechanics | proven | Live model-experience parity still depends on approved release run evidence and solver-role prompt tests. |
| Reflective dataset carries target-safe input/output/score/feedback/side-info/provenance and hidden targets reach reflection only through scorer feedback | upstream reflective dataset/ASI adapter; specs `case_visibility`, `gepa_reflection_evidence_visibility` | `GepaReflectiveDataset`, P8 `AimeReflectiveDataset` | agent/LM byte-identical example tests, P8 target-isolation tests | gap | Add matrix row proof for parse failures/format failures and generic source projection outside P8 bridge. |
| Detailed GEPA result/report exposes candidates, lineage, validation aggregate/subscores, frontier membership, metric calls, attempts, skip policy, events, and best validation candidate | upstream `GEPAResult.from_state`; spec sections 6, 11, 21 | `GepaReport`, `Optimized::optimizer_report::<GepaReport>()`, P8 JSON | GEPA report tests and P8 JSON tests | gap | Add public optimize-path assertion for all required report fields, not only direct optimizer report. |
| Phase events/progress expose GEPA-specific phases without parsing generic engine events | upstream callbacks; spec sections 4, 14, 21 | `GepaEventSummary`, P8 projection | GEPA event tests and P8 event JSON mapping | gap | Add order assertion for seed validation, iteration, parent selected, minibatch sampled, parent evaluated, skip/proposal, child, acceptance, validation, frontier. |
| Durable resume restores optimizer, sampler/selector state, cache, compatibility fingerprints, and does not repeat committed evaluations | upstream state save; AIME spec sections 3 and 4 | `GepaCheckpointState`, engine checkpoints, `leaven-run` compatibility manifest | checkpoint state tests; run-dir compatibility tests; `optimizer_compatibility_fingerprint_includes_checkpointed_strategy_state` | gap | Add GEPA-specific resume proof for parent/part/next-batch and seed-validation non-repeat. |
| AIME/P8 live operator path discloses profile, models, dataset/source counts, cache/resume, budget, baseline/optimized validation/test numbers, and deltas versus GEPA CAIS targets | GEPA CAIS artifact and `examples/aime_math`; AIME spec sections 1-2 | `examples/p8_aime_gepa/src/main.rs`, README/AGENTS | deterministic and cache-only proof classification tests | gap | Run only with approved provider spend; do not claim paper parity from deterministic or cache-only runs. |
| DSPy default parity is not claimed until merge and DSPy trace/feedback defaults exist or are labeled disabled | upstream `dspy.GEPA`; spec section 3.2 | future adapter/report label | report/spec language | intentional-delta | Keep all current claims labeled core GEPA or optimize-anything AIME, not DSPy-default parity. |

## Current Priority

The first implementation pass should close rows that make the algorithm
observable and self-checking without provider spend:

1. Prompt snapshot/diff tests for the claimed reflection profiles.
2. GEPA resume proof that does not repeat seed validation or sampler state.
3. Full-validation cache-hit and resume-no-repeat seed validation tests.
4. Public optimize-path assertion for the full detailed GEPA report.
5. Broader GEPA phase-order assertion covering acceptance, validation, and frontier update.
