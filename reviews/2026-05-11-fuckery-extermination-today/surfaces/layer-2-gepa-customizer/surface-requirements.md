# Layer 2 GEPA Surface Requirements

Status: canonical Layer 2 audit document.

This file is the exact GEPA customizer contract a future implementor should
code against. It is area-specific to `leaven-gepa` and adjacent vocabulary
crates. It does not replace Layer 1 `optimize(...)` ergonomics or Layer 3
optimizer-author contracts.

## Layer Boundary

Layer 2 users want GEPA, but not necessarily default GEPA. They should touch
algorithm knobs, not engine internals. The public/private spec names those
knobs as surface, parent selector, part selector, batch sampler,
reflector/proposer, acceptance, population/frontier, validation cadence, merge,
and stopping (`docs/specs/gepa_public_private_surface.md:172-189`). It also
says this layer may expose strategy traits but should not force users to build
engine trust/read scopes or evaluation request templates
(`docs/specs/gepa_public_private_surface.md:207-208`).

The ordinary builder supplies defaults; it does not remove the slots
(`docs/specs/gepa_public_private_surface.md:753-769`).

## Global Invariants

These invariants apply to every slot.

- Graph mutation goes through `RunContext` or one invariant-preserving engine
  finalizer. Strategies do not mutate graph internals.
- Strategies receive scoped views or selected evidence/rendered payloads, not
  forbidden split data or broad global stores.
- Train/search evidence may drive reflection, selection, acceptance, and
  population by default. Validation and test are hidden from reflection by
  default, with test final-report-only (`docs/specs/gepa_optimizer_surface.md:364-384`).
- Evidence is not preference. Acceptance and population must not collapse
  arbitrary evidence into scalar scores unless using a scalar-specific
  implementation (`docs/specs/initial_library.md:1382-1445`).
- Trace is opaque; rendering is the bridge to downstream consumers
  (`docs/specs/guiding_principles.md:321-323`).
- Every costful stage reports cost through budget mechanisms. Reflection LM or
  agent work is proposer/reflection cost, not hidden evaluator cost
  (`docs/specs/gepa_optimizer_surface.md:528-533`).
- Every non-derivable private decision state participates in checkpoint/restore
  (`docs/specs/gepa_optimizer_surface.md:543-550`).
- Placeholder names are not public capability names. Scaffolding is named as
  scaffolding or kept out of ordinary exports.

## Slot Contract Table

| Slot | Public Layer 2 name | Request must include | Response/error | Private state | Current state | Required proof |
| --- | --- | --- | --- | --- | --- | --- |
| Editable surface | `EditSurface` via `.surface(surface)` | artifact type and chosen surface are configured; part projection happens through surface | surface parts, part view, or `SurfaceError`; surface edit lowers to artifact change | surface config fingerprint, if stateful | `EditSurface` is healthy and pure (`crates/leaven-surface/src/edit_surface.rs:7-127`) | surface laws plus GEPA reflector receives selected part view |
| Parent selection | `ParentSelector` | population view/state, scoped graph view, selection context, RNG/search state if needed | parent id(s), no-parent decision, or `ParentSelectionError`; selection outcome observation hook | RNG/cursor/weights if non-derivable | public `CandidateSelector` returns generic `Selection`; default `ParetoFrequencyWeighted` returns best (`crates/leaven-gepa/src/selector.rs:34-104`) | deterministic best and stochastic frequency-weighted tests are separate |
| Part selection | `PartSelector` | selected parent id, selected artifact, surface, part list/view handles, selected feedback/evidence refs, optional attribution | part id(s) or `PartSelectionError`/`SurfaceError` | cursor/weights/attribution cache if non-derivable | current trait receives only artifact and surface; `WorstEvidencePart` placeholder (`crates/leaven-gepa/src/part_selector.rs:6-74`) | round-robin test and attributed failing-part test |
| Batch sampling | `BatchSampler` | allowed split/case view, sampler cursor/RNG, minibatch size, budget hint, iteration state | nonempty batch, skip/no-cases decision, or `BatchSamplingError` | RNG/cursor/epoch | no live slot in builder/loop; spec requires it (`docs/specs/gepa_optimizer_surface.md:273-293`) | empty required train/search split rejects before reflection/evaluation |
| Feedback selection | `FeedbackSelector` | parent/child candidates as applicable, assessment ids, casewise outcomes, evidence refs, split/purpose, render limits | selected feedback/evidence refs, selected payload views, rendered handles, or `FeedbackSelectionError` | selection cursor/sampling state if any | no live slot; GEPA retrieves evidence then scalar-projects it (`crates/leaven-gepa/src/optimizer.rs:625-635`) | selected train feedback reaches reflector; validation/test hidden by default |
| Reflection/proposal | `ReflectiveMutation` or `GepaProposer` | parent id, selected part id, part view/render handle, feedback assessment ids, selected evidence/rendered feedback, attribution, lineage, objective/background, proposal count, output mode | surface edit(s) or native proposal(s) with causal inputs and `informed_by`; `GepaProposalError` | LM/provider/cache policy state only through configured runtime; proposer internal state if non-derivable | sync `SurfaceProposer` sees only artifact/surface/part; fixed `ReflectiveMutation` returns one edit (`crates/leaven-gepa/src/proposer.rs:6-47`) | mock LM reflector consumes evidence/trace and emits a proposal through engine finalization |
| Acceptance | `Acceptance` | parent id, child id, screening assessment ids, evidence summaries/refs, split/purpose, preference relation, metric axes, optional validation/admission context | accept/reject/defer with reason, or `AcceptanceError`; no population update | thresholds/adaptive stats if non-derivable | scalar `Gate(f64, f64)` (`crates/leaven-gepa/src/gate.rs:23-27`) | scalar strict improvement plus incomparable/defer cases |
| Validation | `ValidationPolicy` | admitted candidate ids, validation cadence state, split policy, budget snapshot, previous validation summaries | evaluation request intent, skip, or `ValidationPolicyError` | cadence/cursor/state | marker trait only (`crates/leaven-gepa/src/validation.rs:1-16`) | validation-aware policy can influence admission only when explicit and reported |
| Population/frontier | `Population` / `GepaPopulation` | candidate ids, assessment ids, split/purpose, scoped graph/evidence access or selected evidence refs, interpretation policy | population events, best/frontier summary, or `PopulationObservationError`; no graph truth ownership | archive/frontier/fitted model state | scalar-only `GepaPopulation` (`crates/leaven-gepa/src/optimizer.rs:68-81`) | scalar Pareto, no-population, and pairwise tournament all plug in |
| Merge | `MergeScheduler` + merge proposer | frontier/lineage summaries, graph view, iteration/budget, candidate pair(s), surface/read capabilities | merge intent/skip or merge proposal(s) with pair causal provenance; `MergeError` | schedule/cursor/cooldown/RNG | `MergeScheduler` and `SystemAwareMerge` empty placeholders (`crates/leaven-gepa/src/optimizer.rs:720-722`, `crates/leaven-gepa/src/proposer.rs:54-56`) | merge records pair causal provenance and routes through proposal finalizer |
| Stopping/config | `Stopper` / `GepaConfig` | iteration, budget snapshot, optimizer state summary, validation cadence state, callback stop flags | continue/done with reason, or `StopError` | counters/patience if not derivable | `max_iterations` exists; `GepaConfig` is placeholder (`crates/leaven-gepa/src/optimizer.rs:298-303`, `crates/leaven-gepa/src/optimizer.rs:716-718`) | report contains stop reason; restore preserves next stop decision |

## Required Public Names

Keep as Layer 2 names:

- `ParentSelector`
- `PartSelector`
- `BatchSampler`
- `FeedbackSelector`
- `ReflectiveMutation` for real reflection only
- `GepaProposer` if the proposal surface is split from reflection rendering
- `Acceptance`
- `ValidationPolicy`
- `Population` / `ParetoFrontier`
- `MergeScheduler`
- `Stopper`

Acceptable lower-level/internal names:

- `CandidateSelector`, only where no proposal-parent relationship is implied;
- scalar helper names inside an `Acceptance` implementation;
- fixed/deterministic proposer fixture names in tests/examples.

Hard-cut names:

- fixed-edit `ReflectiveMutation` must be renamed or moved;
- public GEPA `Gate` becomes `Acceptance` or an internal scalar helper;
- `ParetoFrequencyWeighted` cannot name deterministic best-parent selection;
- `WorstEvidencePart` cannot be exported until it receives attributed evidence;
- empty renderer/materializer structs cannot be presented as standard reflection
  pieces.

Evidence: nomenclature rules in `docs/specs/initial_library.md:531-572`,
GEPA parent/part naming in `docs/specs/gepa_public_private_surface.md:246-287`,
and current divergent exports in `crates/leaven-gepa/src/lib.rs:10-25`.

## Request/Response Details

### `ParentSelectionRequest`

Must include:

- population view or state handle;
- scoped `RunGraphView`;
- current iteration;
- optional RNG/search context;
- allowed split/search state if the selector is split-aware;
- previous selection outcome when observing feedback.

Returns:

- one parent candidate id;
- multiple parent ids only for merge/variadic selectors;
- typed no-parent decision;
- `ParentSelectionError`.

Must not:

- mutate graph;
- run evaluators;
- read forbidden splits;
- call LMs or subprocesses.

The initial spec's ideal selector includes population view, graph view,
selection context, typed result, and outcome observation
(`docs/specs/initial_library.md:3367-3377`). Current `CandidateSelector` lacks
selection context and error shape (`crates/leaven-gepa/src/selector.rs:34-40`).

### `PartSelectionRequest`

Must include:

- selected parent id;
- selected artifact;
- surface;
- projected part list or lazy part access;
- selected feedback/evidence refs;
- optional `AttributableEvidence<S::PartId>` already checked for the same
  surface fingerprint;
- current iteration and batch identity.

Returns:

- one or more part ids;
- typed no-editable-part decision;
- `PartSelectionError` or `SurfaceError`.

Must not:

- lower edits;
- mutate artifact;
- call LMs;
- silently choose a part when attribution names a part outside this surface.

Evidence: casewise measurement and attribution are deliberately separate, and
trace-aware selectors consume attribution
(`docs/specs/initial_library.md:1370-1380`). The current `PartSelector` cannot
receive attribution (`crates/leaven-gepa/src/part_selector.rs:6-13`).

### `BatchSamplingRequest`

Must include:

- allowed train/search partition or case view;
- stable case ids;
- minibatch size/policy;
- iteration and epoch/cursor;
- budget hint;
- split policy.

Returns:

- nonempty batch for policies that require feedback cases;
- skip/no-cases decision for policies that allow no-case operation;
- `BatchSamplingError`.

Must not:

- bypass split policy;
- duplicate cases unless the policy says replacement is allowed;
- sample validation/test under default policy.

Evidence: the step contract requires batch sampling before parent feedback
evaluation (`docs/specs/gepa_optimizer_surface.md:320-341`), and validation/test
visibility is constrained (`docs/specs/gepa_optimizer_surface.md:364-384`).

### `FeedbackSelectionRequest`

Must include:

- candidate ids relevant to the next reflection/acceptance decision;
- assessment ids;
- case ids/outcomes when casewise;
- evidence refs or scoped evidence payload views;
- split role and evaluation purpose;
- render budget/size limits;
- policy for validation/test visibility.

Returns:

- selected evidence refs;
- selected feedback strings and trace excerpts when allowed;
- optional attribution view;
- rendered handles for reflection;
- `FeedbackSelectionError`.

Must not:

- copy evidence payloads into population state;
- leak hidden validation/test content into reflection by default;
- collapse feedback/trace into scalar before reflection.

Evidence: `ScoredFeedbackEvidence` preserves feedback and trace
(`crates/leaven-evidence/src/feedback.rs:8-43`), but GEPA currently projects to
scalar (`crates/leaven-gepa/src/optimizer.rs:57-65`).

### `GepaMutationRequest`

Must include:

- parent candidate id;
- selected surface part id;
- selected part view or render handle;
- feedback assessment ids;
- selected evidence/rendered feedback;
- optional attribution for the selected part;
- lineage summary;
- objective/background;
- proposal count;
- output mode: surface edits, native proposals, or either.

Returns:

- metered surface edits;
- metered native proposals;
- typed parse/validation/proposal errors;
- explicit empty-output decision only if the contract allows skip.

Must not:

- apply proposals directly;
- record graph mutations directly;
- hide parse errors as empty output;
- require a concrete LM provider in `leaven-gepa`;
- depend on Python GEPA API compatibility.

Evidence: standard reflective mutation and ASI inputs are specified in
`docs/specs/gepa_optimizer_surface.md:445-488`; Python GEPA compatibility and
dict-shaped candidates are explicit non-goals
(`docs/specs/gepa_optimizer_surface.md:771-779`).

### `AcceptanceRequest`

Must include:

- parent and child candidate ids;
- screening/minibatch assessment ids;
- comparable summaries or evidence refs;
- configured preference relation/metric axes;
- split role and evaluation purpose;
- optional validation/admission state;
- budget snapshot if decision can trigger more work.

Returns:

- `Accept { reason }`;
- `Reject { reason }`;
- `Defer { reason, validation_intent }`;
- `AcceptanceError`.

Must not:

- update population;
- request hidden test evidence;
- mutate graph;
- erase incomparable evidence as numeric zero.

Evidence: minimum `Acceptance` input/output/must-not contract is in
`docs/specs/gepa_public_private_surface.md:523-530`. The current `Gate` is only
`fn decide(parent_score: f64, candidate_score: f64)`
(`crates/leaven-gepa/src/gate.rs:23-27`).

### `PopulationObservationRequest`

Must include:

- candidate id(s);
- assessment id(s);
- split role and evaluation purpose;
- scoped graph/evidence access or selected evidence refs;
- population-specific interpretation policy.

Returns:

- `PopulationEvent`s;
- updated best/frontier summary;
- `PopulationObservationError`.

Must not:

- own graph truth;
- persist copied evidence payloads unless explicitly part of population state;
- select next parent as a side effect;
- assume scalar casewise evidence for all populations.

Evidence: population is live optimizer state, not selection policy
(`docs/specs/initial_library.md:1447-1451`). Current GEPA population observation
is scalar-only (`crates/leaven-gepa/src/optimizer.rs:68-81`) while pairwise
tournament population state exists (`crates/leaven-population/src/tournament.rs:78-146`).

## Error Contract

Every public slot gets a structured error enum or struct. Strings are acceptable
only as display fields inside structured variants or at very outer product
boundaries.

Minimum error categories:

- configuration errors: missing surface, missing reflector/default path, invalid
  split policy, unsupported merge config;
- selection errors: no parent, no editable part, attribution/surface mismatch;
- sampling errors: empty required split, duplicate/invalid case selection;
- evidence errors: missing assessment, forbidden split, unsupported evidence
  shape, render budget exceeded;
- reflection/proposal errors: LM/runtime failure, parse failure, invalid surface
  edit, no proposal when proposals are required;
- acceptance errors: incomparable evidence under a policy that requires an
  ordering, missing metric axis, forbidden validation/test read;
- checkpoint errors: missing graph truth for checkpointed candidate ids,
  incompatible schema, invalid restored private state.

Current code already has examples of structured checkpoint refusal for missing
graph truth in GEPA checkpoint/restore (`crates/leaven-gepa/src/optimizer.rs:517-531`),
but many Layer 2 slot failures currently collapse into `SurfaceError::Message`
or `OptimizerError::Message` (`crates/leaven-gepa/src/optimizer.rs:552-568`,
`crates/leaven-gepa/src/optimizer.rs:629-631`).

## State And Checkpoint Gates

Each slot must declare one of:

- stateless: checkpoint state is `()`;
- derivable: state is recomputed from graph/events and declares the derivation;
- explicit: state is serialized in GEPA private state.

Required explicit state:

- parent selector RNG/cursor/weights;
- part selector cursor/weights/attribution cache when non-derivable;
- batch sampler epoch/cursor/RNG;
- acceptance/admission adaptive thresholds or validation pending state;
- validation cadence/patience;
- merge schedule/cooldown/RNG;
- stopper patience/counters;
- population/frontier/fitted preference state.

Current checkpoint proof is partial: the reduced `GepaCheckpointState` snapshots
population, parent selector, part selector, and gate (`crates/leaven-gepa/src/optimizer.rs:199-211`),
and tests cover selector cursor and frontier membership
(`crates/leaven-gepa/tests/gepa_smoke.rs:180-263`). The full Layer 2 contract
requires the same proof for the full slot set.

## Implementation Proof Gates

Minimum proof set before Layer 2 can be called product-ready:

- `leaven-gepa` builder exposes every Layer 2 slot and rejects invalid config
  before run start.
- Fixed-edit fixtures are not named or exported as `ReflectiveMutation`.
- A mock LM/agent reflector receives selected part view, feedback, trace, and
  assessment refs, then produces a proposal through engine finalization.
- Feedback/trace from `ScoredFeedbackEvidence` reaches reflection before scalar
  projection.
- Validation/test split content is hidden from reflection by default.
- Parent selection and part selection are separately tested and named.
- Acceptance uses a structured request and reasoned decision; scalar strict
  improvement is one implementation.
- Population observation is evidence-shape-neutral enough for scalar Pareto,
  no-population, and pairwise tournament modes.
- Merge, when enabled, records pair causal provenance.
- Checkpoint/restore preserves every stateful slot's next decision.
- Reports expose selected parents, proposals, evidence refs, acceptance reasons,
  frontier state, cost, and stop reason, matching observability requirements
  (`docs/specs/guiding_principles.md:341-347`).

Canonical verification commands for the implementation slice:

```bash
cargo nextest run -p leaven-gepa
cargo nextest run -p leaven-population
cargo nextest run -p leaven-preference
cargo nextest run -p leaven-render
cargo run -p p3_gepa_parity
just check
```

`just check` remains the completion gate (`docs/specs/gepa_optimizer_surface.md:753-769`).
