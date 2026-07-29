## Boundary
This crate owns lowered evaluation data: datasets, cases, split membership, split-use policy, and report vocabulary.

It does not execute evaluations. Evaluator traits, registries, trust checks, case-set resolution, graph records, and runtime scheduling belong in `leaven-engine`; product builders that lower user train/validation/test inputs belong in `leaven-run`.

## Routing
- `src/dataset.rs` owns durable case collections, source-row case metadata
  lowering, ordered source-row manifests, and membership fingerprints.
- `src/split.rs` owns partition roles, overlap policy, known-case validation,
  exact stratified split construction over caller-supplied strata, split
  construction over caller-supplied row-order ranges, exact caller-declared
  split manifests, split fingerprinting, and `CaseSetVersion` attachment.
- `src/sampler.rs` owns deterministic lowered case sampling state over known
  case IDs. It may retain cursor/RNG-like state for checkpointing, but it must
  not call evaluators or inspect artifacts.
- `src/use_policy.rs` owns which split roles may drive proposer feedback, selection, acceptance, population observation, report output, or final test.
- `src/report.rs` owns report data shapes after evaluation has happened; it must not reach back into engine graph mutation.
- Missing-but-owned surface from the eval audit belongs here when implemented:
  evaluation plans, request templates, suites, stable work-input traits, and
  graph/evidence-ref report vocabulary. Keep those as lowered data contracts,
  not executor loops.

## Local Bait
- `SplitRole::Test` can be report/final-test data, but it is not optimizer training data unless an explicit `FinalTestPolicy` exception says so. Keep this boundary visible when adding benchmark behavior.
- `Dataset` fingerprints currently track membership/order, not full input payload content. Do not cite them as content hashes unless the implementation is changed and tested.
- `Case::from_source_row` preserves upstream ordered-manifest identity as
  operational metadata (`source_row_index`, `source_id`) while keeping the
  Leaven `CaseId` numeric and row-stable. It does not parse dataset files or
  turn source metadata into optimizer-visible semantic features.
- `SourceRowManifest` preserves caller-supplied upstream row order and unique
  source IDs, fingerprints that row identity, and lowers rows into stable
  `Case::from_source_row` datasets. It does not parse Parquet/CSV/JSON files,
  hash row payload content, discover splits, or certify benchmark provenance.
- Engine tests may construct `EvaluationSet` and `CaseSet` directly; do not move that execution machinery here.
- `SplitUsePolicy::gepa_train_val_test` states intent. Product builders must
  lower at least optimizer-hidden `TEST` into engine `TrustPolicy`, and engine
  trust must refuse resolved case IDs that belong to hidden partitions
  (including `EvaluationSet::Cases`). Do not claim full split-use enforcement
  until purpose/`SplitUsePolicy` coupling is also engine-readable.
- `leaven-run` currently constructs `Dataset`, `DatasetSplits`, engine
  `CaseSet`, trust policy, final evaluation requests, and reports locally. If a
  second product builder or GEPA path starts copying that stack, stop and move
  the reusable lowered vocabulary here first.
- `CategoryRoundRobinSampler` is a deterministic category-keyed sampler for
  EvoSkill-style train pools. It cycles category groups and per-category case
  cursors without replacement until each pool wraps. It is not a stratified
  split builder and does not certify category provenance.
- `StratifiedSplitBuilder` constructs disjoint train/validation/test or custom
  role membership from caller-declared strata and exact counts. It does not
  discover strata, certify paper category provenance, choose benchmark counts,
  or make `SplitRole::Test` optimizer-visible.
- `RowOrderSplitBuilder` constructs disjoint split membership from a
  caller-declared ordered case manifest and half-open row ranges. It exists for
  upstreams that define splits as row slices, such as Trace2Skill's
  SpreadsheetBench `0:200` / `200:400` split. It does not parse dataset files,
  infer semantic splits, or execute benchmark runners.
- `ExplicitSplitBuilder` constructs disjoint split membership from exact
  caller-declared role membership over a known case universe. It exists for
  papers that publish or require explicit case-id manifests. It does not locate
  missing manifests, infer categories, or certify that a substitute is faithful.

## Decision Cards
- when: adding stable task/case inputs or split semantics
  do: define the durable data shape here, then let `leaven-run` lower ordinary builder inputs into it
  preserve: duplicate/missing case refusal, disjoint-default split policy, stable fingerprints, and explicit final-test exceptions
  avoid: encoding benchmark semantics as vector positions or magic partition strings in examples/GEPA
  verify: run `cargo test -p leaven-eval --test split_contract` and the `leaven-run` builder test that consumes it

- when: adding deterministic case samplers
  do: keep inputs and outputs as `CaseId` plus lightweight grouping metadata,
  preserve checkpointable sampler state, and reject ambiguous pools before
  sampling
  preserve: no evaluator execution, no graph mutation, and no hidden
  dataset-provenance claims
  avoid: moving optimizer-specific acceptance/frontier policy into this crate
  verify: run `cargo test -p leaven-eval --test category_sampler`

- when: adding reports
  do: keep reports as post-evaluation data with graph IDs, evidence refs, split roles, and summary projections
  preserve: absent/failed evidence as distinct from numeric zero, and final-test-only markers as report semantics
  avoid: making `report.rs` call evaluators, inspect `RunGraph` internals, or flatten hidden payloads into ordinary reports
  verify: run `cargo test -p leaven-eval -p leaven-run`

## Proof Anchors
- `cargo test -p leaven-eval` proves dataset IDs, split overlap policy,
  unknown-case refusal, exact stratified split construction, split
  fingerprints, explicit paper-manifest lowering, deterministic sampler state,
  and train/validation/test use-policy boundaries.
- `cargo test -p leaven-run` proves product builders can consume lowered eval vocabulary without making this crate own builder ergonomics.
- `cargo test -p leaven-engine --test engine_contract case_set_resolution` proves execution-time case-set resolution stays in the engine layer.
