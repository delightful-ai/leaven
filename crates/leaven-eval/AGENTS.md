## Boundary
This crate owns lowered evaluation data: datasets, cases, split membership, split-use policy, and report vocabulary.

It does not execute evaluations. Evaluator traits, registries, trust checks, case-set resolution, graph records, and runtime scheduling belong in `leaven-engine`; product builders that lower user train/validation/test inputs belong in `leaven-run`.

## Routing
- `src/dataset.rs` owns durable case collections and membership fingerprints.
- `src/split.rs` owns partition roles, overlap policy, known-case validation, split fingerprinting, and `CaseSetVersion` attachment.
- `src/use_policy.rs` owns which split roles may drive proposer feedback, selection, acceptance, population observation, report output, or final test.
- `src/report.rs` owns report data shapes after evaluation has happened; it must not reach back into engine graph mutation.
- Missing-but-owned surface from the eval audit belongs here when implemented:
  evaluation plans, request templates, suites, stable work-input traits, and
  graph/evidence-ref report vocabulary. Keep those as lowered data contracts,
  not executor loops.

## Local Bait
- `SplitRole::Test` can be report/final-test data, but it is not optimizer training data unless an explicit `FinalTestPolicy` exception says so. Keep this boundary visible when adding benchmark behavior.
- `Dataset` fingerprints currently track membership/order, not full input payload content. Do not cite them as content hashes unless the implementation is changed and tested.
- Engine tests may construct `EvaluationSet` and `CaseSet` directly; do not move that execution machinery here.
- `SplitUsePolicy::gepa_train_val_test` states intent; current engine trust
  enforcement is not yet a complete lowering of that policy. Do not claim split
  policy is enforced until hidden explicit-case and resolved-set tests prove it
  at the engine boundary.
- `leaven-run` currently constructs `Dataset`, `DatasetSplits`, engine
  `CaseSet`, trust policy, final evaluation requests, and reports locally. If a
  second product builder or GEPA path starts copying that stack, stop and move
  the reusable lowered vocabulary here first.

## Decision Cards
- when: adding stable task/case inputs or split semantics
  do: define the durable data shape here, then let `leaven-run` lower ordinary builder inputs into it
  preserve: duplicate/missing case refusal, disjoint-default split policy, stable fingerprints, and explicit final-test exceptions
  avoid: encoding benchmark semantics as vector positions or magic partition strings in examples/GEPA
  verify: run `cargo nextest run -p leaven-eval --test split_contract` and the `leaven-run` builder test that consumes it

- when: adding reports
  do: keep reports as post-evaluation data with graph IDs, evidence refs, split roles, and summary projections
  preserve: absent/failed evidence as distinct from numeric zero, and final-test-only markers as report semantics
  avoid: making `report.rs` call evaluators, inspect `RunGraph` internals, or flatten hidden payloads into ordinary reports
  verify: run `cargo nextest run -p leaven-eval -p leaven-run`

## Proof Anchors
- `cargo nextest run -p leaven-eval` proves dataset IDs, split overlap policy, unknown-case refusal, split fingerprints, and train/validation/test use-policy boundaries.
- `cargo nextest run -p leaven-run` proves product builders can consume lowered eval vocabulary without making this crate own builder ergonomics.
- `cargo nextest run -p leaven-engine --test case_set_resolution` proves execution-time case-set resolution stays in the engine layer.
