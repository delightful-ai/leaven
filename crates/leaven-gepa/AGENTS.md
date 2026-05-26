## Boundary
This crate owns reusable GEPA optimizer behavior: optimizer loop state, surface-edit lowering, parent/candidate selection, part selection, admission gates, validation policy, reflective mutation, merge scheduling, and GEPA checkpoint state.

It composes core, surface, engine, evidence, population, render, and LM vocabulary. It must not become the engine, a provider crate, a workspace backend, or the place for generic population/frontier implementations.

## Routing
- `src/optimizer.rs` owns GEPA loop state, checkpoint/restore shape, selected train partition, observed candidates, and population observation through `GepaPopulation`.
- `src/selector.rs`, `src/part_selector.rs`, `src/gate.rs`, and `src/validation.rs` own GEPA-specific strategy policy.
- `src/proposer.rs` owns GEPA reflection/proposal helpers that are
  provider-neutral; concrete LM/provider lowering belongs in `leaven-lm-*` or
  agent crates. The module is private; behavior-bearing types are curated at
  the crate root, and scaffolds route through `test_support`.
- The legacy `AgentBacked` GEPA bootstrap/proposer scaffold is deleted. Do not
  reintroduce a GEPA-specific stage route until it materializes the artifact
  under reflection and has product-facing tests.
- Surface ownership is explicit: GEPA selects a part from an `EditSurface` and lowers edits through that surface into artifact-native changes. Artifact-specific surfaces belong in `leaven-surface` or `leaven-artifact-*`.
- The current reference loop is: full-validate the seed into
  `GepaReferenceState`, select a parent from the validation frontier frequency
  map, resolve the TRAIN partition through `RunContext`, sample an explicit
  epoch-shuffled minibatch, evaluate parent and child on the same minibatch,
  accept only strict minibatch improvements, full-validate accepted children,
  and admit validated candidates back into `GepaReferenceState`. Train
  population observation is still maintained as an internal adapter/ablation
  surface, not as reference parent selection truth.
- Reference parent selection and epoch-shuffled minibatch sampling share GEPA's
  upstream `random.Random(seed)` draw order. The Python-compatible RNG is
  reachable through an opaque `#[doc(hidden)]` parameter type only because the
  public `BatchSampler` extension hook needs to name it; it is optimizer replay
  state, not an ordinary Leaven random facility.
- Reflection remains single-part by default: select one surface part, build the
  reflective dataset once from the full parent row set via the configured
  `ReflectiveDatasetBuilder`, assemble one `ReflectRequest`, call a
  `GepaReflector` with that request, apply the returned proposal batch through
  `RunContext`, then update GEPA state. `FixedSurfaceEdit` lives under
  `leaven_gepa::test_support`; it is a scaffold reflector, not a product
  reflection path. Agent-backed reflection must materialize the artifact and
  route through `RunContext::propose` before `apply_batch`.
- Reflection is build-once-pass-down: `reflect_candidate` receives a fully
  built `ReflectRequest` and never projects its own data. Do not reintroduce a
  reflector that derives feedback inside itself.

## Local Bait
- Engine tests use local optimizer wrappers; do not move GEPA selector, gate, or checkpoint private state into `leaven-engine` to make those tests shorter.
- `leaven-lm` is a neutral vocabulary dependency here, not permission to place OpenAI/Anthropic request fields or CLI/session behavior in GEPA.
- Population defaults such as `ParetoFrontier` and `KeepBest` are consumed here; reusable population behavior still belongs in `leaven-population`.
- The fixed-edit fixture is `test_support::FixedSurfaceEdit`. It is scaffolding
  for GEPA extension slots, not production reflection. Do not re-export or
  document it as production GEPA reflection.
- Product-facing GEPA proof requires slot contracts for candidate selection,
  part selection, feedback/evidence rendering, reflection/proposal, acceptance,
  validation, population, merge, stopping, and checkpoint state. Topology and
  scalar-score improvement are not sufficient.
- `PopulationBestFallback` is an explicit advanced fallback/ablation selector.
  It is not reference GEPA Pareto selection. The reference parent selector reads
  `GepaReferenceState` validation-frontier frequency state in the optimizer
  loop.
- Public GEPA nomenclature matters. `CandidateSelector`, `PartSelector`,
  `BatchSampler`, `Acceptance`, `ValidationPolicy`,
  `Population`/`ParetoFrontier`, and `Merge` are the teachable slot names.
  `CandidateSelector` and `Gate` can be internal implementation words only if
  public-facing APIs do not teach them as the GEPA slots.
- `ReflectiveMutation`, `ReflectiveMutationConfig`, `SystemAwareMerge`,
  `GepaConfig`, `MergeScheduler`, `SelectedFeedback`, `ReflectiveFeedbackRecord`,
  `GepaReflectionEvidence`, and `WorstEvidencePart` were inert, misleading, or
  superseded public names and have been removed. `WorstEvidencePart` was a
  `PartSelector` placeholder struct with no trait impl; reintroduce trace-aware
  part selection only as a behavior-bearing, tested selector. `SelectedFeedback`
  collapsed into `ReflectRequest` (`examples` plus `source_refs`); the
  reflective dataset unit is `ReflectiveCase` with one or more `ReflectiveRun`
  records; per-case scoring is row-local through `GepaCaseEvidence`, and the
  dataset selection/projection seam is `ReflectiveDatasetBuilder` with the
  `GepaReflectiveDataset` default. Do not reintroduce the removed names.

## Proof Anchors
- `cargo nextest run -p leaven-gepa` proves local GEPA surface ownership, edit lowering, selectors, gates, checkpoint/restore, validation, and proposer read-scope behavior.
- `cargo nextest run -p leaven-gepa --test gepa_contract` is the focused local
  gate for the current GEPA contract suite. Its `gepa_smoke` module proves
  surface lowering, fixed-edit proposer behavior, train-filtered population,
  checkpoint state, and hidden validation visibility tests.
- `cargo nextest run -p leaven --test gepa_parity` proves the public P3 workflow:
  explicit edit-surface GEPA, train-filtered Pareto updates, and best-candidate
  result. `FixedSurfaceEdit` in that proof is not product proof of GEPA
  reflection.
- `cargo test -p leaven --test topology_contract` proves GEPA stays outside cold core and retains the expected dependency shape.

## Decision Cards
- when: replacing fixed-edit reflection
  do: route through `GepaReflector::reflect_candidate(ctx, surface, request)` with a pre-built `ReflectRequest`; the optimizer builds the reflective dataset once via `ReflectiveDatasetBuilder`; agent-backed reflectors must use `RunContext::propose` before `apply_batch`
  preserve: build-once-pass-down (a reflector never projects its own data), causal parent provenance plus `informed_by` refs from `ReflectRequest::informed_by`, hidden validation/test defaults, typed proposal/reflection errors, and engine finalization semantics
  avoid: widening `SurfaceProposer<A, S>` in place as if artifact/surface/part is enough context, letting a reflector derive feedback internally, or letting GEPA read provider-specific LM fields
  verify: run `cargo nextest run -p leaven-gepa --test gepa_contract`

- when: changing what data reflection sees
  do: implement or swap a `ReflectiveDatasetBuilder` (named type or closure); the builder receives every parent assessment row id, not one bundled assessment; `GepaReflectiveDataset` is the GEPA-parity default and requires `P::Case: ReflectiveCaseInput` plus row-local projectable evidence, or `GepaReflectiveDataset::with_case_input(...)` for an explicit target-safe projection
  preserve: the builder as the single selection seam, separate from backend presentation (LM renderer vs agent workspace materialization), and full row provenance in each example/source ref
  avoid: keying projection on the evidence type, merging selection and presentation into one seam, or relying on `Display` for a whole target-bearing case envelope
  verify: run `cargo nextest run -p leaven-gepa --test gepa_contract`

- when: adding or renaming GEPA strategy slots
  do: give each slot a request type, output type, structured error, private/checkpoint state story, budget/cost behavior, event/report behavior where relevant, and explicit hidden-split rules
  preserve: GEPA as one optimizer value over shared engine/eval/evidence/render/population seams
  avoid: moving slot state into `leaven-engine`, exporting empty config structs as capability, or collapsing evidence/preference/population into two `f64`s
  verify: run `cargo nextest run -p leaven-gepa --test gepa_contract` plus `cargo test -p leaven --test topology_contract` if manifests or exports change

- when: changing population, acceptance, or selection logic
  do: keep scalar strict-improvement as one default adapter, not the trait signature
  preserve: casewise evidence shape until a strategy explicitly interprets it, and keep population as optimizer-private live state
  avoid: treating `CaseAssessmentEvidence.output()` or `CaseAssessmentEvidence.feedback()` as discardable before reflection and part selection have had a chance to consume them
  verify: run `cargo nextest run -p leaven-gepa --test gepa_contract`
