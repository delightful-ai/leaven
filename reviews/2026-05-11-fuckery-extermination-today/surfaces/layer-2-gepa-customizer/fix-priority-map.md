# Layer 2 GEPA Fix Priority Map

Status: canonical Layer 2 audit document.

This is an ordered fix map for GEPA/customizer work. It is not a compatibility
plan. Every item assumes hard cutover semantics.

## Priority 0: Quarantine False Public Proof

Fix: remove production-looking names from fixed or empty scaffolding before
expanding the API.

Scope:

- fixed-edit `ReflectiveMutation`;
- public placeholder `ReflectiveMutationConfig`, `SystemAwareMerge`,
  `GepaConfig`, `MergeScheduler`, and `WorstEvidencePart`;
- renderer/materializer names exported without behavior when used as proof of
  GEPA reflection.

Evidence:

- deterministic proposer is allowed only as Milestone A scaffolding
  (`docs/specs/gepa_optimizer_surface.md:692-713`);
- current `ReflectiveMutation` stores one edit and ignores artifact/surface/part
  (`crates/leaven-gepa/src/proposer.rs:21-47`);
- AIME uses fixed `ReflectiveMutation` as its reflector
  (`examples/p8_aime_gepa/src/main.rs:81-94`);
- placeholders are publicly re-exported from `leaven-gepa`
  (`crates/leaven-gepa/src/lib.rs:10-25`);
- render structs are exported but empty (`crates/leaven-render/src/lib.rs:10-15`,
  `crates/leaven-render/src/prompt.rs:1-3`,
  `crates/leaven-render/src/surface.rs:1-3`,
  `crates/leaven-render/src/run_graph.rs:1`,
  `crates/leaven-render/src/materializer.rs:1-5`).

Proof gate:

- `cargo nextest run -p leaven-gepa` still passes with fixed-edit fixtures under
  explicit fixture names;
- no ordinary GEPA example claims real reflection unless a reflector consumes
  selected evidence/trace context;
- public exports/preludes do not expose empty placeholder capability names as
  standard GEPA behavior.

Why first: as long as false public proof remains, later work can accidentally
validate the proxy again.

## Priority 1: Lock The Slot Names And Builder Surface

Fix: make the public Layer 2 builder enumerate the actual GEPA strategy slots:
surface, parent selector, part selector, batch sampler, feedback
selection/rendering, reflector/proposer, acceptance, validation policy,
population/frontier, merge, and stopper/config.

Evidence:

- Layer 2 slot names are specified in the public/private surface doc
  (`docs/specs/gepa_public_private_surface.md:172-189`);
- `Gepa::builder()` is required to support those knobs and related run controls
  (`docs/specs/gepa_optimizer_surface.md:273-293`);
- current builder exposes only surface, optional population, and reflector
  (`crates/leaven-gepa/src/optimizer.rs:663-713`);
- current low-level constructor exposes only a subset through
  `with_strategies` (`crates/leaven-gepa/src/optimizer.rs:246-272`);
- `candidate_selector` is explicitly not the GEPA-facing name
  (`docs/specs/gepa_optimizer_surface.md:137-151`).

Correction:

- public GEPA trait name is `ParentSelector`, not `CandidateSelector`;
- public admission trait name is `Acceptance`, not `Gate`;
- feedback selection/rendering is a first-class GEPA subslot between evaluation
  and reflection, even if it is implemented as a default renderer helper;
- `ParetoFrequencyWeighted` is kept only if it actually implements stochastic
  frequency-weighted frontier sampling;
- `SelectBestCandidate` becomes `SelectBestParent` in GEPA-facing API if kept;
- builder methods must return incomplete-configuration types or a final
  `build()` error that names the missing/contradictory slot.

Proof gate:

- compile/API tests demonstrate builder calls for every required slot;
- a public/private mapping test or doc example shows the distinction between
  Layer 1 inputs, Layer 2 slot methods, and lowered/private engine contracts;
- negative tests prove missing surface, missing reflector/default reflection
  path, invalid split/validation, empty required train/search set, and merge
  without support fail before run start (`docs/specs/gepa_optimizer_surface.md:295-304`);
- `cargo nextest run -p leaven-gepa` covers builder defaults and explicit
  customizer paths.

## Priority 2: Define Request, Response, Error, And State For Every Slot

Fix: every slot gets a request type, response type, structured error, private
state/checkpoint story, event/report behavior, and `must not` rules.

Evidence:

- minimum slot contracts are already specified at a high level
  (`docs/specs/gepa_public_private_surface.md:504-537`);
- checkpointed GEPA private state must include RNG, sampler cursor,
  parent/part selector state, admission state, merge scheduler, and population
  when not derivable (`docs/specs/gepa_optimizer_surface.md:543-550`);
- current checkpoint state covers only the reduced live slot set
  (`crates/leaven-gepa/src/optimizer.rs:199-211`).

Correction:

- `ParentSelectionRequest` -> parent id(s) or typed no-parent decision;
- `PartSelectionRequest` -> part id(s) or typed surface/selection error;
- `BatchSamplingRequest` -> nonempty batch or typed no-cases decision;
- `FeedbackSelectionRequest` -> selected assessment/evidence refs and rendered
  handles;
- `GepaMutationRequest` -> surface edits or native proposals with provenance;
- `AcceptanceRequest` -> accept/reject/defer with reason;
- `ValidationPolicyRequest` -> validation intent or skip;
- `PopulationObservationRequest` -> population events and updated private state;
- `MergeScheduleRequest` -> merge intent or skip;
- `StopRequest` -> continue/done reason.

Proof gate:

- trait contract tests use only public request/response/error types;
- stateful slot resume tests prove next decision equality after checkpoint and
  restore;
- each slot contract names its allowed context/capabilities and the
  lowered/private contract it composes with;
- errors are structured enums/structs rather than strings at public slot
  boundaries.

## Priority 3: Restore Evidence And Trace Flow Before Reflection

Fix: introduce explicit feedback/evidence selection and rendering between
evaluation and reflection. Do not collapse `ScoredFeedbackEvidence` to scalar
before the reflector can use feedback and trace.

Evidence:

- reflection renderer inputs include assessment IDs, evidence, attribution,
  lineage, and objective/background (`docs/specs/gepa_optimizer_surface.md:460-473`);
- ASI/feedback sources include evaluator evidence, casewise outcomes,
  attribution, command output, transcripts, errors, and summaries
  (`docs/specs/gepa_optimizer_surface.md:475-483`);
- `ScoredFeedbackEvidence` stores feedback and trace (`crates/leaven-evidence/src/feedback.rs:8-43`);
- current GEPA projection keeps only scalar scores
  (`crates/leaven-gepa/src/optimizer.rs:57-65`);
- the live reflector receives no evidence (`crates/leaven-gepa/src/proposer.rs:6-18`).

Correction:

- `FeedbackSelector` chooses which assessments/outcomes/traces are visible to
  reflection under split policy;
- `ReflectionRenderer` renders selected part view, selected feedback, trace
  excerpts, lineage, objective/background, and allowed error summaries;
- selected evidence refs remain in provenance through `InfoRef`;
- validation/test content is hidden from reflection by default.

Proof gate:

- mock reflector test asserts the selected feedback text and trace lines it
  receives are exactly those allowed by the split policy;
- trace-attributed part selector test proves attribution can drive part choice;
- hidden validation/test split test proves no validation/test feedback reaches
  reflection under default policy.

## Priority 4: Route Reflection Through Engine Finalization

Fix: replace synchronous `SurfaceProposer` as the product path with an async
GEPA proposer/reflector that finalizes through `RunContext::propose` or an
equivalent single engine finalizer.

Evidence:

- engine `Proposer<P>` is async and typed by associated request
  (`crates/leaven-engine/src/stage/proposer.rs:27-46`);
- `ProposalContext` exposes graph, read scope, budget, render context, and
  materialize context (`crates/leaven-engine/src/context/proposal_context.rs:8-62`);
- `RunContext::propose` records proposer calls through the uniform event/cost
  path (`crates/leaven-engine/src/context/run_context.rs:191-208`);
- current GEPA calls local `propose_edit`, then directly records/applies a
  proposal batch (`crates/leaven-gepa/src/optimizer.rs:560-593`);
- proposal generation must fail before graph mutation if budget is exhausted
  (`docs/specs/gepa_optimizer_surface.md:528-533`).

Correction:

- `GepaMutationRequest` carries selected parent, selected part, part view/render
  handle, selected evidence refs/rendered feedback, proposal count, and output
  mode;
- output supports surface edits and artifact-native proposals;
- GEPA lowers surface edits and preserves causal/informational provenance;
- all costful reflection work charges proposer/reflection budget, not metric
  calls unless evaluation actually happened.

Proof gate:

- budget-exhausted reflection test leaves graph unchanged;
- proposer event tests match engine proposer semantics;
- proposal provenance test asserts parent causal input and assessment/evidence
  `informed_by` refs are present.

## Priority 5: Make Selection, Batch, Acceptance, And Population Evidence-Honest

Fix: replace scalar-only signatures with request shapes that carry the evidence,
preference, split, and graph context each strategy actually needs.

Evidence:

- parent and part selection are distinct public concepts
  (`docs/specs/gepa_public_private_surface.md:246-287`);
- `PartSelector` minimum input includes optional attributed evidence
  (`docs/specs/gepa_public_private_surface.md:523-527`);
- evidence is not preference, and scores are only one evidence shape plus one
  preference relation (`docs/specs/initial_library.md:1382-1445`);
- current `PartSelector` receives only artifact and surface
  (`crates/leaven-gepa/src/part_selector.rs:6-13`);
- current `Gate` receives two `f64`s (`crates/leaven-gepa/src/gate.rs:23-27`);
- current `GepaPopulation` observes only scalar casewise evidence
  (`crates/leaven-gepa/src/optimizer.rs:68-81`);
- `TournamentPopulation` already proves richer pairwise population state exists
  outside GEPA (`crates/leaven-population/src/tournament.rs:78-146`).

Correction:

- `ParentSelector` reads population view, graph view, and selection context, and
  can observe selection outcome;
- `PartSelector` reads selected parent/artifact/surface plus attribution or
  selected feedback;
- `BatchSampler` samples allowed train/search cases and returns typed no-cases;
- `Acceptance` reads parent/child ids, screening assessments, evidence summaries
  or refs, split/purpose, preference relation, and validation/admission context;
- `Population` observes candidate ids, assessment ids, split/purpose, graph or
  evidence access, and strategy-specific interpretation policy.

Proof gate:

- parent selector tests cover best-parent and real frequency-weighted sampling
  separately;
- part selector tests cover round-robin and attributed failure selection;
- batch sampler tests cover deterministic epoch order, empty required splits,
  and hidden validation/test partitions;
- acceptance tests include scalar improvement, equality/regression, incomparable
  evidence, and defer;
- GEPA population tests cover scalar Pareto, no-population, and pairwise
  tournament integration without changing the loop.

## Priority 6: Implement Validation, Merge, Stopper, And Checkpoint Discipline

Fix: finish the currently missing live slots that decide evidence visibility,
multi-parent provenance, run stopping, and resumability.

Evidence:

- default split behavior hides validation/test from reflection by default
  (`docs/specs/gepa_optimizer_surface.md:364-384`);
- first product-grade GEPA requires validation/admission, explicit population
  updates, summaries, budget, and trust scopes (`docs/specs/gepa_optimizer_surface.md:552-573`);
- defaults include minibatch, held-out validation/test, disabled merge, and
  proposal count (`docs/specs/gepa_optimizer_surface.md:575-589`);
- merge canonicalization requires pair causal provenance
  (`docs/specs/initial_library.md:3519-3529`);
- current `ValidationPolicy` is only a marker and merge/config are placeholders
  (`crates/leaven-gepa/src/validation.rs:1-16`,
  `crates/leaven-gepa/src/optimizer.rs:716-722`).

Correction:

- `ValidationPolicy` decides when validation may influence admission and reports
  whether it did;
- `MergeScheduler` schedules a merge proposer with pair causal provenance;
- `Stopper` composes iteration, budget, callbacks, validation cadence, and
  optimizer state summary into a done/continue reason;
- checkpoint state includes every non-derivable private decision state;
- validation, merge, and stopping emit reportable reasons rather than silent
  booleans or placeholder marker types.

Proof gate:

- validation-hidden-by-default test;
- validation-aware-explicit-policy test;
- merge proposal lineage test with `CausalInputs::Pair`;
- stop reason report test;
- mid-run checkpoint/restore test for sampler, selectors, acceptance, merge,
  stopper, and population.

## Priority 7: Restore End-To-End Proof Without Proxy Paths

Fix: only after the slot contracts are real, restore examples as proof of the
library surface.

Evidence:

- user acceptance path asks for a GEPA example that works with the higher-level
  new surface and can run AIME with mocked LM while being swappable to OpenAI
  with minimal change (`reviews/2026-05-11-fuckery-extermination-today/complaints/session-user-messages-for-codex.md:198-206`);
- current deterministic AIME proof shows score movement but uses fixed
  `ReflectiveMutation` (`examples/p8_aime_gepa/src/main.rs:81-99`) and a live
  OpenAI shell escape (`examples/p8_aime_gepa/src/main.rs:271-310`);
- completion verification for GEPA work is `just check`
  (`docs/specs/gepa_optimizer_surface.md:753-769`).

Correction:

- AIME-like proof uses product builder, mock LM reflector through Leaven LM
  vocabulary, selected evidence/trace rendering, cache/runtime roles, and GEPA
  strategy slots;
- swapping mock LM to OpenAI is provider construction, not an architecture
  change;
- old deterministic fixture examples are explicitly low-level plumbing proofs,
  not product GEPA proofs.

Proof gate:

- `cargo run -p p3_gepa_parity` remains a thin deterministic loop/plumbing proof
  only if renamed/scoped accordingly;
- AIME or equivalent runs with a mock LM reflector and asserts feedback-driven
  edit generation;
- `cargo nextest run -p leaven-gepa`;
- `cargo nextest run -p leaven-population`;
- `cargo nextest run -p leaven-preference`;
- `cargo nextest run -p leaven-render`;
- `just check`.
