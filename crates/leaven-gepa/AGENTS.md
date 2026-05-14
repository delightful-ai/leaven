## Boundary
This crate owns reusable GEPA optimizer behavior: optimizer loop state, surface-edit lowering, parent/candidate selection, part selection, admission gates, validation policy, reflective mutation, merge scheduling, and GEPA checkpoint state.

It composes core, surface, engine, evidence, population, render, and LM vocabulary. It must not become the engine, a provider crate, a workspace backend, or the place for generic population/frontier implementations.

## Routing
- `src/optimizer.rs` owns GEPA loop state, checkpoint/restore shape, selected train partition, observed candidates, and population observation through `GepaPopulation`.
- `src/selector.rs`, `src/part_selector.rs`, `src/gate.rs`, and `src/validation.rs` own GEPA-specific strategy policy.
- `src/proposer.rs` owns GEPA reflection/proposal helpers that are provider-neutral; concrete LM/provider lowering belongs in `leaven-lm-*` or agent crates.
- Surface ownership is explicit: GEPA selects a part from an `EditSurface` and lowers edits through that surface into artifact-native changes. Artifact-specific surfaces belong in `leaven-surface` or `leaven-artifact-*`.
- The current live loop is: select parent from population, evaluate train
  partition casewise, project evidence to scalar scores, select a surface part,
  call a `GepaReflector`, apply the returned proposal batch through
  `RunContext`, then update population. `FixedSurfaceEdit` is still a
  scaffold reflector; agent-backed reflection must route through
  `RunContext::propose` before `apply_batch`.

## Local Bait
- Engine tests use local optimizer wrappers; do not move GEPA selector, gate, or checkpoint private state into `leaven-engine` to make those tests shorter.
- `leaven-lm` is a neutral vocabulary dependency here, not permission to place OpenAI/Anthropic request fields or CLI/session behavior in GEPA.
- Population defaults such as `ParetoFrontier` and `KeepBest` are consumed here; reusable population behavior still belongs in `leaven-population`.
- The fixed-edit fixture is `FixedSurfaceEdit`. The name is deliberately plain:
  it is scaffolding for GEPA's `Reflect` type parameter, not reflection. Do not
  re-export or document it as production GEPA reflection.
- Product-facing GEPA proof requires slot contracts for parent selection, part
  selection, feedback/evidence rendering, reflection/proposal, acceptance,
  validation, population, merge, stopping, and checkpoint state. Topology and
  scalar-score improvement are not sufficient.
- Public GEPA nomenclature matters. `ParentSelector`, `PartSelector`,
  `BatchSampler`, `Acceptance`, `ValidationPolicy`,
  `Population`/`ParetoFrontier`, and `Merge` are the teachable slot names.
  `CandidateSelector` and `Gate` can be internal implementation words only if
  public-facing APIs do not teach them as the GEPA slots.
- `ReflectiveMutation`, `ReflectiveMutationConfig`, `SystemAwareMerge`,
  `GepaConfig`, and `MergeScheduler` were inert or misleading public names and
  have been removed. Do not reintroduce them until they carry behavior, errors,
  state, and tests.

## Proof Anchors
- `cargo nextest run -p leaven-gepa` proves local GEPA surface ownership, edit lowering, selectors, gates, checkpoint/restore, validation, and proposer read-scope behavior.
- `cargo nextest run -p leaven-gepa --test gepa_smoke` is the focused local gate
  for the current scaffold: surface lowering, fixed-edit proposer behavior,
  train-filtered population, checkpoint state, and hidden validation visibility
  tests.
- `cargo nextest run -p leaven-gepa --test agent_stage_routing` proves the
  agent-backed GEPA reflection slot: selected feedback refs enter
  `ReflectRequest`, the fake runtime writes `output/proposal.json`, the parser
  returns a proposal batch, `RunContext::propose` records the batch, and
  `apply_batch` creates the candidate.
- `cargo nextest run -p leaven --test gepa_parity` proves the public P3 workflow:
  explicit edit-surface GEPA, train-filtered Pareto updates, and best-candidate
  result. `FixedSurfaceEdit` in that proof is not product proof of GEPA
  reflection.
- `cargo test -p leaven --test topology_contract` proves GEPA stays outside cold core and retains the expected dependency shape.

## Decision Cards
- when: replacing fixed-edit reflection
  do: route through `GepaReflector` with `ReflectRequest`/`SelectedFeedback`; agent-backed reflectors must use `RunContext::propose` before `apply_batch`
  preserve: causal parent provenance plus `informed_by` assessment/evidence refs, hidden validation/test defaults, typed proposal errors, and engine finalization semantics
  avoid: widening `SurfaceProposer<A, S>` in place as if artifact/surface/part is enough context, or letting GEPA read provider-specific LM fields
  verify: run `cargo nextest run -p leaven-gepa --test agent_stage_routing`, then `cargo nextest run -p leaven-gepa --test gepa_smoke`

- when: adding or renaming GEPA strategy slots
  do: give each slot a request type, output type, structured error, private/checkpoint state story, budget/cost behavior, event/report behavior where relevant, and explicit hidden-split rules
  preserve: GEPA as one optimizer value over shared engine/eval/evidence/render/population seams
  avoid: moving slot state into `leaven-engine`, exporting empty config structs as capability, or collapsing evidence/preference/population into two `f64`s
  verify: run `cargo nextest run -p leaven-gepa --test gepa_smoke` plus `cargo test -p leaven --test topology_contract` if manifests or exports change

- when: changing population, acceptance, or selection logic
  do: keep scalar strict-improvement as one default adapter, not the trait signature
  preserve: casewise evidence shape until a strategy explicitly interprets it, and keep population as optimizer-private live state
  avoid: treating `ScoredFeedbackEvidence.feedback()` or trace lines as discardable before reflection and part selection have had a chance to consume them
  verify: run `cargo nextest run -p leaven-gepa --test gepa_smoke`
