# Layer 2 GEPA Vision Comparison

Status: canonical Layer 2 audit document.

This compares the original GEPA/customizer vision against current repo state.
It uses shared refinement docs as background only; the evidence below is from
the governing specs, complaints archive, and live code.

## Short Answer

Original vision: GEPA should be a Rust-native optimizer value with swappable
strategy slots. It should feel like GEPA to a power user, while using Leaven's
typed artifact/surface/evidence/preference/population/rendering/engine substrate
instead of Python GEPA's dict/string adapter shape.

Current reality: the repo has a useful GEPA loop scaffold, typed surfaces,
casewise scalar Pareto state, and checkpoint hooks for a reduced slot set. It
does not yet have the Layer 2 GEPA customizer surface. Reflection is a fixed
edit fixture, feedback/trace is dropped before reflection, acceptance and
population are scalar-only, builder slots are incomplete, and several public
names imply behavior that does not exist.

The correction is not to make GEPA the whole library or to put GEPA hooks into
the engine. The correction is to make `leaven-gepa` consume the shared substrate
through honest GEPA-specific strategy contracts.

## Original Vision Anchors

The initial spec has three relevant user tiers:

- ordinary users call `optimize(seed).train(...).score(...).using(Gepa...)` and
  should not understand every internal trait (`docs/specs/initial_library.md:451-468`);
- GEPA customizers replace one GEPA part through `.parent_selector(...)`,
  `.surface(...)`, `.part_selector(...)`, `.batch_sampler(...)`,
  `.acceptance(...)`, `.population(...)`, and `.merge(...)`
  (`docs/specs/initial_library.md:470-486`);
- optimizer authors implement their own optimizer over `Optimizer` and
  `RunContext`, and must not be forced into GEPA's sequence
  (`docs/specs/initial_library.md:487-509`).

The user-message archive says the same thing in product terms: map each
interactable GEPA aspect, keep public/private surfaces distinct, preserve
power-user swappability, and make the spec clear enough that a smart implementor
does not invent a nearby proxy (`reviews/2026-05-11-fuckery-extermination-today/complaints/session-user-messages-for-codex.md:105-125`).

The final thesis is the anchor:

> A Rust optimizer is a configured value that drives a typed run graph by
> proposing changes to artifacts, requesting assessments, interpreting evidence
> through preference relations, and maintaining live populations, while the
> engine provides budgeted, observable, capability-scoped execution.

Evidence: `docs/specs/initial_library.md:4749-4759`.

## Comparison Table

| Area | Ideal GEPA/customizer contract | Current implementation | Blocker/gap | Correction direction | Required proof |
| --- | --- | --- | --- | --- | --- |
| GEPA scope | GEPA is one optimizer, composed from GEPA-specific strategies, not the engine or whole library (`docs/specs/initial_library.md:406-443`). | `Gepa` implements an optimizer loop with some strategy fields (`crates/leaven-gepa/src/optimizer.rs:176-197`). | The implemented subset can be mistaken for complete GEPA because public names suggest finished slots. | Keep GEPA rhythm in `leaven-gepa`; consume engine/evidence/render/population seams instead of duplicating or elevating GEPA concepts. | A non-GEPA optimizer can still use shared engine/evidence/population primitives without GEPA-specific hooks. |
| Rust-native candidate shape | Replace Python GEPA dict/string candidates with `P::Artifact`, `EditSurface`, `PartId`, `View`, `Edit`, and artifact-native `Change` (`docs/specs/gepa_optimizer_surface.md:102-135`). | `EditSurface` preserves typed part ids, addresses, borrowed views, edits, fingerprints, and pure `change_part` lowering (`crates/leaven-surface/src/edit_surface.rs:7-127`). | Surface is healthy, but GEPA does not pass the selected part view to reflection. | Keep `leaven-surface` as the Rust-native replacement; pass part view/render handle through `GepaMutationRequest`. | Contract test proves a reflector sees selected part id and view before producing an edit. |
| Builder/customizer surface | Builder supports surface, population, parent selector, part selector, batch sampler, reflector, reflection LM, acceptance, validation, merge, run limits, proposal count, history, split policy (`docs/specs/gepa_optimizer_surface.md:273-293`). | Builder exposes surface, optional population, and reflector only (`crates/leaven-gepa/src/optimizer.rs:663-713`). | Power users cannot configure promised slots through the Layer 2 surface. | Add builder methods only after slot request/response/error/state contracts exist. | API tests cover every builder slot and rejection of incomplete or contradictory config (`docs/specs/gepa_optimizer_surface.md:295-304`). |
| Parent selection | GEPA-facing name is `ParentSelector`; it chooses which candidate/program version to mutate next (`docs/specs/gepa_public_private_surface.md:246-287`, `docs/specs/gepa_optimizer_surface.md:137-151`). | Public trait is `CandidateSelector`, and `ParetoFrequencyWeighted` delegates to population best candidate (`crates/leaven-gepa/src/selector.rs:34-40`, `crates/leaven-gepa/src/selector.rs:79-104`). | Name and behavior diverge from the paper-facing contract. | Hard cutover to public `ParentSelector`; rename deterministic best selection or implement real frequency weighting. | Tests distinguish deterministic best-parent from stochastic Pareto-frequency sampling. |
| Part selection | Part selection chooses where inside the selected parent to edit and can consume attributed evidence for trace-aware selectors (`docs/specs/initial_library.md:3476-3500`). | `PartSelector` receives only artifact and surface; `WorstEvidencePart` is placeholder only (`crates/leaven-gepa/src/part_selector.rs:6-13`, `crates/leaven-gepa/src/part_selector.rs:72-74`). | Trace-aware part selection is impossible through the current trait. | `PartSelectionRequest` includes parent id, artifact/surface, selected feedback/evidence, and optional attribution. | `InvokedAndFailingPart`-style test selects an attributed failing part; round-robin remains a paper-baseline implementation. |
| Batch sampling | GEPA samples feedback minibatches from the allowed train/search split before parent/child screening (`docs/specs/gepa_optimizer_surface.md:320-341`). | The loop evaluates `EvaluationSet::Partition(self.train_partition.clone())` with a hard-coded `TRAIN` partition and no sampler slot (`crates/leaven-gepa/src/optimizer.rs:192-193`, `crates/leaven-gepa/src/optimizer.rs:612-620`). | Minibatch policy, empty-split behavior, replacement policy, and hidden validation/test guarantees are not customizable. | `BatchSampler` owns case selection from scoped split/case views and returns a nonempty batch, skip, or typed no-cases error. | Tests cover epoch/minibatch selection, empty required train/search rejection, and validation/test exclusion by default. |
| Reflection/proposal | Reflective mutation consumes parent, selected part, part view, assessment IDs, evidence, attribution, lineage, objective/background, and produces provider-neutral LM input (`docs/specs/gepa_optimizer_surface.md:445-483`). | `SurfaceProposer` is sync and receives only artifact/surface/part (`crates/leaven-gepa/src/proposer.rs:6-18`). Fixed `ReflectiveMutation` always returns one edit (`crates/leaven-gepa/src/proposer.rs:21-47`). | Real LM/agent reflection cannot be implemented without forking. | Async `ReflectiveMutation`/`GepaProposer` over `GepaMutationRequest` plus scoped context; fixture renamed or moved. | Mock LM reflector consumes rendered feedback/trace and part view, then emits proposal with `informed_by` refs. |
| Engine proposal finalization | Proposer stages use typed requests, `ProposalContext`, budget/read/render/materialize context, and `RunContext::propose` for uniform events/costs (`docs/specs/initial_library.md:2174-2233`; `crates/leaven-engine/src/context/run_context.rs:191-208`). | GEPA calls local proposer then directly records/applies the batch (`crates/leaven-gepa/src/optimizer.rs:560-593`). | GEPA owns a parallel proposal path and hard-coded cost shape. | GEPA-specific output adapter is okay; finalization must flow through engine proposer semantics. | Budget-exhausted proposal test fails before graph mutation; proposal events match engine proposer path. |
| Feedback/evidence flow | Feedback sources include evaluator fields, casewise outcomes, attribution, command output, transcripts, errors, and summaries (`docs/specs/gepa_optimizer_surface.md:475-483`). | `ScoredFeedbackEvidence` stores feedback and trace (`crates/leaven-evidence/src/feedback.rs:8-43`), but GEPA projects it to scalar casewise scores (`crates/leaven-gepa/src/optimizer.rs:57-65`). | Feedback and trace exist in types but are discarded before reflection and selection. | Add feedback/evidence selection and rendering before reflection; preserve assessment ids/evidence refs. | Test proves natural-language feedback and trace from scoring reach the reflector under train/search policy. |
| Acceptance | Acceptance interprets parent/child evidence summaries and metric axes, returning a reasoned decision without updating population or reading hidden test evidence (`docs/specs/gepa_public_private_surface.md:523-530`). | `Gate` compares two `f64` scores and returns accept/reject (`crates/leaven-gepa/src/gate.rs:6-27`). | Pairwise, listwise, multi-axis, validation-aware, and defer decisions cannot plug in. | Public slot is `Acceptance` over candidate ids, assessments/evidence refs, split/purpose, preference relation, and validation/admission context. | Acceptance contract tests cover scalar strict improvement, regression, equivalent, incomparable, and defer. |
| Population/frontier | Population is live optimizer state, not selection policy; no-frontier, tournament, and user-defined frontier must be valid configurations (`docs/specs/initial_library.md:1447-1451`; `docs/specs/guiding_principles.md:127-139`). | `GepaPopulation` observes only scalar casewise evidence (`crates/leaven-gepa/src/optimizer.rs:68-81`), while richer `TournamentPopulation` exists outside GEPA (`crates/leaven-population/src/tournament.rs:78-146`). | GEPA cannot use richer population strategies without adapters or loop changes. | Population observation receives candidate/assessment ids, split/purpose, and scoped graph/evidence access; scalar is one implementation. | GEPA population tests cover scalar Pareto, no-population, and pairwise tournament without changing the loop. |
| Validation/test split behavior | Train/search drives reflection; validation is held out unless explicit policy; test is final-report-only by default (`docs/specs/gepa_optimizer_surface.md:364-384`). | `evaluate_casewise` always evaluates `EvaluationSet::Partition(self.train_partition.clone())` with default `TRAIN` (`crates/leaven-gepa/src/optimizer.rs:612-620`); `ValidationPolicy` is marker-only (`crates/leaven-gepa/src/validation.rs:1-16`). | Validation and test are not live GEPA customizer contracts. | Implement `BatchSampler`, `ValidationPolicy`, and split policy with explicit feedback visibility. | Tests prove validation/test feedback is hidden from reflection by default and visible only under explicit validation-aware policy. |
| Merge | Merge is a scheduled proposer with pair causal provenance and surface-edit lowering (`docs/specs/initial_library.md:3519-3529`). | `MergeScheduler` and `SystemAwareMerge` are empty placeholders (`crates/leaven-gepa/src/optimizer.rs:720-722`, `crates/leaven-gepa/src/proposer.rs:54-56`). | Public names imply merge exists while no contract does. | Merge is another proposer schedule with pair request, cost, provenance, and checkpointed schedule state. | Merge test records `CausalInputs::Pair` and applies through the same proposal finalization path. |
| Stopping/config | Stop behavior is one GEPA strategy decision alongside budget, validation cadence, and optimizer state summaries (`docs/specs/gepa_public_private_surface.md:753-769`). | `max_iterations` exists, but `GepaConfig` is a placeholder and stop output is not a reasoned slot (`crates/leaven-gepa/src/optimizer.rs:298-303`, `crates/leaven-gepa/src/optimizer.rs:716-718`). | Runs can stop, but customizers cannot replace or inspect a stop policy as a Layer 2 decision. | `Stopper` returns continue/done with a reason and checkpointed private counters/patience when needed. | Stop reason report test plus checkpoint/restore test for the next stop decision. |
| Checkpoint/private state | Non-derivable state for RNG, sampler, selectors, gate/admission, merge, and population must checkpoint (`docs/specs/gepa_optimizer_surface.md:543-550`). | Current checkpoint includes reduced fields only (`crates/leaven-gepa/src/optimizer.rs:199-211`); tests cover selector cursor and frontier membership (`crates/leaven-gepa/tests/gepa_smoke.rs:180-263`). | Future slots have no state contract yet. | Every slot contract includes private state, derivability, and checkpoint laws before public export. | Resume test verifies same next parent, part, batch, validation cadence, merge schedule, stopper decision, and frontier after restore. |

## Current Healthy Substrate

`leaven-surface` is not the Layer 2 root blocker. Its `EditSurface` shape
preserves typed artifact projections, part identity, address, borrowed view,
surface-native edit, fingerprint, and pure lowering into artifact-native
changes (`crates/leaven-surface/src/edit_surface.rs:7-127`). The failure is that
GEPA does not carry the selected part view and evidence context onward.

The population crate also contains real ingredients beyond scalar GEPA:
`TournamentPopulation` tracks pairwise Bradley-Terry-style state and observes
pairwise evidence (`crates/leaven-population/src/tournament.rs:78-146`). The
failure is that GEPA's population trait cannot consume it as a GEPA population.

The engine proposer substrate exists and is closer to the vision than the GEPA
local proposer path: async `Proposer<P>`, typed request, `ProposalContext`, and
`RunContext::propose` are live (`crates/leaven-engine/src/stage/proposer.rs:27-46`,
`crates/leaven-engine/src/context/proposal_context.rs:8-62`,
`crates/leaven-engine/src/context/run_context.rs:191-208`). GEPA should consume
that substrate or preserve its laws through one equivalent finalizer.

## Nomenclature Classification

Not every divergence from Python GEPA is a bug. These are justified Rust-native
divergences: typed `Artifact` plus `EditSurface` instead of `dict[str, str]`;
`Assessment` and `Evidence` instead of scalar-only score; `Renderer` and
`Materializer` instead of a universal reflective dataset.

These are acceptable lower-level names but wrong public GEPA names:
`CandidateSelector` can remain internal or generic, but Layer 2 GEPA exposes
`ParentSelector` because the selected candidate becomes causal parent of the
next proposal (`docs/specs/gepa_public_private_surface.md:278-287`).

These are false or premature public names until behavior exists:
`Gate` for the public admission slot, fixed-edit `ReflectiveMutation`,
`WorstEvidencePart`, `ParetoFrequencyWeighted` while deterministic best-only,
empty renderer/materializer structs in ordinary proof paths, and public
`GepaConfig`/`MergeScheduler` placeholders. The hard-cut fix is to rename,
implement, or quarantine them under explicit fixture/scaffold modules.

## Required Interpretation

Do not solve these gaps by adding broad engine internals to GEPA customizers.
The original vision separates responsibilities:

- `leaven-gepa` owns GEPA rhythm and strategy slots;
- `leaven-engine` owns graph mutation, budget, events, trust/read scope, cache,
  and finalizing context methods;
- `leaven-surface` owns typed editable projections;
- `leaven-evidence` owns evidence shapes;
- `leaven-preference` owns evidence interpretation;
- `leaven-population` owns reusable live population state;
- `leaven-render` owns opaque-to-visible bridges;
- `leaven-run` owns ordinary product-builder lowering.

That is the hard-cut direction. Compatibility wrappers around the current
fixture-shaped traits would preserve the wrong mental model.
