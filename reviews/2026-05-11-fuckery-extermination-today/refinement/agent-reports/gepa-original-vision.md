# GEPA Original Vision Refinement Report

## Short Answer

GEPA is aligned with the original Leaven vision as a spec target, but the current implementation is not aligned yet, and the first-pass audit is only partially sharp enough. The audit correctly identifies the biggest Layer 2 failures: fixed reflection, lost feedback/trace context, scalar-only acceptance/population, placeholder slots, and public names that imply more than they do. The refinement it still needs is to frame these failures against the original promise: GEPA is one optimizer value with swappable strategy slots, not the whole library, not a second engine, and not a public wrapper around engine internals.

The deterministic-proposer milestone is allowed scaffolding only when it is named as a deterministic proposer and used to prove the reusable GEPA loop. It becomes fake public reflection when it is called `ReflectiveMutation`, exposed as the reflector path, or used as evidence that Leaven has real GEPA.

## Findings

### GOV-001: The Audit Should Make GEPA's Scope Smaller And Sharper

`id`: GOV-001

`severity`: high

`vision promise`:

GEPA is one optimizer value, not the engine and not the whole Leaven library. The original spec says Leaven should support GEPA-style reflective prompt evolution while also supporting MIPRO, TextGrad/Trace, MAP-Elites, pairwise tournaments, agentic proposers, and future optimizers; it explicitly says the cold core must not assume every optimizer has GEPA's loop shape. It then narrows GEPA itself to one configured optimizer made from GEPA-specific strategies: parent selector, part selector, batch sampler, reflector/proposer, acceptance, validation policy, population/frontier, and optional merge proposer.

`current audit coverage`:

The Layer 2 audit asks the right question: can a power user swap GEPA strategy slots without forking GEPA or losing necessary context? It also lists the correct broad slot set and says the current implementation provides only a beginning of a swappable value shape.

`gap`:

The audit reads as a catalog of GEPA implementation smells more than as a scope correction. It should explicitly say that fixing GEPA means making one optimizer naturally express the GEPA paper shape on top of the shared substrate. It should not let GEPA-specific failures become arguments for changing engine ownership, moving GEPA concepts into core, or treating GEPA parity as proof of the entire optimizer library.

`correction`:

Add a short "GEPA Scope" subsection to the integrated refinement docs: GEPA owns its rhythm and strategy slots; `leaven-engine` owns graph/budget/trust/callback execution; `leaven-run` owns product-builder lowering; `leaven-surface`, `leaven-evidence`, `leaven-render`, `leaven-population`, and `leaven-preference` provide reusable vocabulary. GEPA must consume those seams, not duplicate them or pull them upward.

Evidence:

- `docs/specs/initial_library.md:406-408` says Leaven supports GEPA but is not GEPA-only.
- `docs/specs/initial_library.md:410-423` lists assumptions the cold core must not make, including GEPA loop shape.
- `docs/specs/initial_library.md:443` defines GEPA as one optimizer value composed from smaller GEPA-specific strategies.
- `docs/specs/initial_library.md:601-653` separates engine infrastructure from optimizer rhythm.
- `docs/specs/initial_library.md:4751-4759` gives the final thesis: optimizer smart, engine budgeted/observable/capability-scoped.
- `reviews/2026-05-11-fuckery-extermination-today/surfaces/layer-2-gepa-customizer/agent-report-layer-2.md:15-34` states the Layer 2 question and current short answer.

### GOV-002: Deterministic Proposer Scaffolding Is Allowed, But `ReflectiveMutation` As Public Proof Is Not

`id`: GOV-002

`severity`: blocker

`vision promise`:

The GEPA optimizer surface explicitly stages Milestone A as a "Real GEPA Loop, Deterministic Proposer": `Gepa` implements `Optimizer<P>`, the deterministic proposer returns a configured surface edit, the builder supports the core strategy slots, and the P3 example becomes thin setup using `Gepa` directly. Milestone B is where `ReflectiveMutation` must use `leaven-lm`, a mock LM, and a standard reflection renderer that consumes casewise evidence and part view.

`current audit coverage`:

The audit correctly flags `ReflectiveMutation` as a fixed-edit fixture and a blocker when used as production-looking reflection. It also flags that GEPA bypasses engine proposer context and passes only artifact/surface/part into `SurfaceProposer`.

`gap`:

The audit does not consistently separate "deterministic proposer used to prove loop plumbing" from "fake public reflective mutation." That distinction matters because otherwise future implementors may overcorrect by banning deterministic proposers entirely, or undercorrect by leaving the fixture in the public reflector path.

`correction`:

Refine every reflection finding to use this rule: deterministic proposer is acceptable only under a name like `FixedEditProposer`, `DeterministicSurfaceEdit`, or test/example fixture, and only for Milestone A. `ReflectiveMutation` must be reserved for Milestone B behavior: async, LM/agent-capable, evidence-aware, rendered-context-driven, and budgeted as proposer/reflection work.

Evidence:

- `docs/specs/gepa_optimizer_surface.md:692-702` allows a deterministic proposer for Milestone A.
- `docs/specs/gepa_optimizer_surface.md:704-713` moves LM vocabulary, mock LM, evidence rendering, and typed proposer feedback to Milestone B.
- `docs/specs/gepa_optimizer_surface.md:445-473` defines standard reflective mutation as renderer/proposer behavior over parent, selected part, part view, assessment IDs, evidence, lineage, and objective/background.
- `crates/leaven-gepa/src/proposer.rs:21-31` calls the current type `ReflectiveMutation` while storing one configured edit.
- `crates/leaven-gepa/src/proposer.rs:40-47` ignores artifact, surface, and part and returns the stored edit.
- `reviews/2026-05-11-fuckery-extermination-today/surfaces/layer-2-gepa-customizer/reflection-and-proposal.md:30-46` correctly records the blocker but should add the milestone distinction.
- `reviews/2026-05-11-fuckery-extermination-today/refinement/vision-comparison.md:135-147` already identifies this distinction and should be folded into the Layer 2 docs.

### GOV-003: The Slot Map Is Conceptually Right, But The Audit Should Separate User-Visible, Customizer, And Lowered Contracts Per Slot

`id`: GOV-003

`severity`: high

`vision promise`:

The public/private GEPA spec already provides the map: ordinary users touch candidate, train/validation/test, scoring/evaluator, runner, `Gepa`, budget, and result; GEPA customizers touch recognizable algorithm knobs; lowered/private contracts include graph insertion, split partitions, evaluator requests, evidence, renderers, proposal stages, population state, budget ledger, callbacks, checkpointing, and reports.

`current audit coverage`:

The audit identifies the right slots: parent selection, part selection, batch sampling, reflection/proposal, acceptance, population/frontier, validation cadence, merge, and stopping. It also points to the current builder exposing only surface/population/reflector and to direct `with_strategies(...)` as lower-level generic plumbing.

`gap`:

The report does not yet force each slot through the three-column contract. As a result, some findings blur whether the missing surface is Layer 1 public ergonomics, Layer 2 customizer strategy API, or Layer 3 engine substrate. Example: "batch sampler missing" is a Layer 2 API gap; "sampled evaluation requests" is lowered/private; `.train(cases)` remains Layer 1.

`correction`:

Add a per-slot refinement table to `strategy-slots.md` with these columns: `GEPA aspect`, `Layer 1 visible surface`, `Layer 2 customizer trait`, `lowered/private contract`, `current state`, and `required correction`. The table should reuse `gepa_public_private_surface.md` section 4 and mark each slot as one of: live and honest, live but too narrow, placeholder, missing.

Evidence:

- `docs/specs/gepa_public_private_surface.md:51-83` defines what Layer 1 users should and should not touch.
- `docs/specs/gepa_public_private_surface.md:172-208` defines Layer 2 customizer knobs and states users still should not build engine trust/read scopes or evaluation request templates.
- `docs/specs/gepa_public_private_surface.md:289-311` provides the interactable GEPA map across user-visible API, customizer API, lowered/private contract, and owner.
- `docs/specs/gepa_public_private_surface.md:504-533` gives minimum strategy contracts for each slot.
- `reviews/2026-05-11-fuckery-extermination-today/surfaces/layer-2-gepa-customizer/strategy-slots.md:10-27` says the builder lacks promised slots.
- `crates/leaven-gepa/src/optimizer.rs:663-713` implements only builder `surface`, `population`, and `reflector`.
- `crates/leaven-gepa/src/optimizer.rs:716-722` leaves `GepaConfig` and `MergeScheduler` as placeholders.

### GOV-004: Parent Selection And Part Selection Are Mapped Correctly In The Audit, But The Corrections Need Literature-Faithful Names

`id`: GOV-004

`severity`: medium

`vision promise`:

GEPA has two different selection questions: parent selection chooses which candidate/program version to mutate next; part selection chooses where inside that candidate's surface to edit. The public GEPA-facing name is `parent_selector`; `candidate_selector` is acceptable internally only when no proposal parent relationship is implied.

`current audit coverage`:

The audit correctly finds that code exposes `CandidateSelector`, that `ParetoFrequencyWeighted` currently returns population best deterministically, and that live part selection cannot see evidence or attribution. It also correctly treats `WorstEvidencePart` as placeholder naming.

`gap`:

The audit should be more explicit that the current name divergence is not justified for GEPA-facing API. `CandidateSelector` is an ML/lower-level term that can be acceptable in generic optimizer internals, but the original Leaven docs deliberately chose `ParentSelector` for GEPA because the selected candidate becomes causal parent of the next proposal. Likewise, `WorstEvidencePart` is worse than a missing implementation because it implies trace-attributed selection while receiving no evidence.

`correction`:

Require a hard cutover in docs and code plan: public GEPA slot becomes `ParentSelector`; deterministic best selection is named `SelectBestParent`; paper-style stochastic frequency sampling keeps `ParetoFrequencyWeighted` only when it actually samples by frontier frequency. Part-selection docs should keep `RoundRobinPart` as paper-baseline reproduction and reserve `InvokedAndFailingPart` or any "evidence" name for selectors that receive attribution or selected feedback.

Evidence:

- `docs/specs/gepa_public_private_surface.md:246-287` explains parent selection versus part selection and reserves `candidate_selector` for lower-level/internal use.
- `docs/specs/gepa_optimizer_surface.md:139-152` repeats the same GEPA-facing naming rule.
- `docs/specs/initial_library.md:531-544` says model-legible naming is infrastructure.
- `docs/specs/initial_library.md:568-569` says `ParentSelector` and `Acceptance` are the preferred names, not `CandidateSelector` and `Gate`.
- `docs/specs/initial_library.md:3362-3377` defines a richer `ParentSelector` over population view, graph view, selection context, and selection outcome.
- `docs/specs/initial_library.md:3432-3457` maps GEPA paper concepts to `ParentSelector`, `PartSelector`, `BatchSampler`, `Acceptance`, and `Population`.
- `crates/leaven-gepa/src/selector.rs:34-40` exposes `CandidateSelector`.
- `crates/leaven-gepa/src/selector.rs:79-104` names `ParetoFrequencyWeighted` but deterministically returns best candidate.
- `crates/leaven-gepa/src/part_selector.rs:6-13` gives part selection only artifact and surface.
- `crates/leaven-gepa/src/part_selector.rs:72-74` exports `WorstEvidencePart` as a placeholder.
- `reviews/2026-05-11-fuckery-extermination-today/surfaces/layer-2-gepa-customizer/agent-report-layer-2.md:362-425` covers parent selection naming and behavior.
- `reviews/2026-05-11-fuckery-extermination-today/surfaces/layer-2-gepa-customizer/agent-report-layer-2.md:296-360` covers evidence-aware part selection.

### GOV-005: Reflection/Proposer Swappability Requires Context Without Leaking Engine Internals

`id`: GOV-005

`severity`: blocker

`vision promise`:

The original vision requires one-shot LM proposers and agentic proposers to work at the same boundary. The framework must carry opaque evidence and traces, then render/materialize them on demand for the consuming stage. A GEPA proposer should receive a typed GEPA mutation request and a scoped proposal context, not raw graph mutation power and not a pre-flattened Python-style reflective dataset.

`current audit coverage`:

The audit correctly says `SurfaceProposer` is too narrow and synchronous, cannot see assessment IDs, evidence payloads, feedback, trace refs, lineage, objective text, render context, materialized context, budget, proposal count, or native proposal output, and bypasses `RunContext::propose(...)`.

`gap`:

The audit's correction direction still leaves two possible paths: engine `Proposer<P>` or an "equally honest" GEPA-local trait. The refinement should decide the contract more tightly: GEPA can own a GEPA-local request/output adapter, but proposal execution should still flow through the engine proposer context/finalization semantics or an adapter that preserves the same event, cost, read-scope, and provenance laws. The customizer should not receive mutable graph internals; it should receive scoped graph/evidence/render capabilities.

`correction`:

Define `ReflectiveMutation` or `GepaProposer` as async over `GepaMutationRequest` plus `ProposalContext`, where the request names selected parent, selected part, feedback assessment IDs or selected evidence refs, proposal count, and output mode. The proposer reads allowed graph/evidence/rendered views through context, returns surface edits or native proposals with `informed_by`, and GEPA lowers/finalizes through `RunContext::propose(...)` or an equivalent single engine finalization path. Do not let GEPA directly record batches after a local synchronous call in product code.

Evidence:

- `docs/specs/guiding_principles.md:104-112` separates rendering from artifact/trace/lineage data.
- `docs/specs/guiding_principles.md:269-273` requires one-shot and agentic proposers to work at the same boundary.
- `docs/specs/guiding_principles.md:321-323` says trace is opaque and rendering is the bridge.
- `docs/specs/initial_library.md:2174-2233` defines async `Proposer<P>` with associated request and `ProposalContext`, and says to use `ctx.propose(...)` when possible.
- `docs/specs/initial_library.md:2740-2760` scopes optimizer, selector, proposer, evaluator, renderer, materializer, agent runtime, and callback capabilities.
- `docs/specs/gepa_optimizer_surface.md:445-487` defines reflection renderer/proposer inputs and rejects a global `oa.log` equivalent in `leaven-gepa`.
- `crates/leaven-engine/src/stage/proposer.rs:28-46` implements the async static proposer trait.
- `crates/leaven-engine/src/context/proposal_context.rs:8-12` holds graph, budget, and read scope for proposer stages.
- `crates/leaven-engine/src/context/proposal_context.rs:27-62` exposes scoped graph, read scope, budget, render context, and materialize context.
- `crates/leaven-engine/src/context/run_context.rs:191-208` records proposer calls through `RunContext::propose`.
- `crates/leaven-gepa/src/proposer.rs:6-18` defines a synchronous surface-only proposer.
- `crates/leaven-gepa/src/optimizer.rs:560-593` calls the GEPA-local proposer, then records and applies a batch directly.
- `reviews/2026-05-11-fuckery-extermination-today/surfaces/layer-2-gepa-customizer/reflection-and-proposal.md:10-28` identifies the engine proposer bypass.
- `reviews/2026-05-11-fuckery-extermination-today/surfaces/layer-2-gepa-customizer/reflection-and-proposal.md:48-65` identifies missing reflection input context.

### GOV-006: Evidence, Acceptance, And Population Need To Preserve Evidence-Shape Neutrality

`id`: GOV-006

`severity`: high

`vision promise`:

Leaven's original vision is evidence-shape neutral. Evidence is not preference; a preference relation interprets evidence. Population is live optimizer state, not graph truth and not parent-selection policy. GEPA's instance Pareto frontier consumes casewise outcomes, while trace-aware selectors consume attribution. Pairwise, listwise, scalar, multi-axis, and mixed evidence must not be faked through scalar averages.

`current audit coverage`:

The audit correctly flags scalar projection from `ScoredFeedbackEvidence`, scalar `Gate`, scalar-only `GepaPopulation`, incomplete preference behavior, and missing feedback/trace selection/rendering before reflection.

`gap`:

The audit should tie these findings to one original-vision violation: the current GEPA loop collapses evidence into scalar averages before the strategy slots that were supposed to interpret evidence. That is not only "too narrow"; it loses the whole evidence/preference/population separation that made Leaven different from Python GEPA.

`correction`:

Use a typed request per strategy slot: `FeedbackSelectionRequest`, `AcceptanceRequest`, and `PopulationObservationRequest`. These should carry candidate IDs, assessment IDs, split/purpose, evidence refs or comparable summaries, and scoped graph/evidence access. Scalar strict improvement remains one default implementation, not the trait signature. GEPA should never drop `ScoredFeedbackEvidence.feedback()` and `trace()` before the reflector has a chance to consume or render them.

Evidence:

- `docs/specs/guiding_principles.md:114-139` requires evidence-shape neutrality and strategy swappability, including no-frontier and tournament configurations.
- `docs/specs/initial_library.md:1370-1380` separates casewise measurement from attribution.
- `docs/specs/initial_library.md:1382-1445` defines `PreferenceRelation` and says scores are one evidence shape plus one preference relation.
- `docs/specs/initial_library.md:1447-1451` defines population as live optimizer state, not selection policy.
- `docs/specs/initial_library.md:2424-2436` keeps fitted preference models on populations and parent selection separate.
- `docs/specs/gepa_public_private_surface.md:523-532` defines minimum inputs/outputs for acceptance, validation, population, and merge slots.
- `crates/leaven-evidence/src/feedback.rs:8-14` stores scalar score, natural-language feedback, and trace in `ScoredFeedbackEvidence`.
- `crates/leaven-evidence/src/feedback.rs:33-43` exposes feedback and trace accessors.
- `crates/leaven-gepa/src/optimizer.rs:57-65` projects `ScoredFeedbackEvidence` down to scalar casewise evidence.
- `crates/leaven-gepa/src/optimizer.rs:68-81` defines `GepaPopulation` over `CasewiseEvidence<ScalarEvidence>`.
- `crates/leaven-gepa/src/gate.rs:23-27` defines `Gate` over two `f64` scores.
- `crates/leaven-gepa/src/optimizer.rs:403-416` gates and updates population from average/scalar evidence only.
- `reviews/2026-05-11-fuckery-extermination-today/surfaces/layer-2-gepa-customizer/agent-report-layer-2.md:228-294` covers feedback and trace dropping.
- `reviews/2026-05-11-fuckery-extermination-today/surfaces/layer-2-gepa-customizer/agent-report-layer-2.md:427-561` covers scalar acceptance and scalar population.

### GOV-007: Batch, Validation, Merge, And Stopping Are Correctly Listed But Under-Audited

`id`: GOV-007

`severity`: medium

`vision promise`:

GEPA's first product-grade implementation must support single-task, train-only, and train+validation runs; explicit test splits as final-report-only; swappable batch sampling; swappable acceptance; swappable validation policy; explicit population updates; GEPA summaries; and a default configuration with minibatch sampling and held-out validation/test behavior. Merge is disabled by default but must be a scheduled proposer when enabled. Stopping is budget/policy-driven and must be visible as a GEPA slot or builder policy.

`current audit coverage`:

The audit lists batch sampler, validation cadence, merge, and stopping as required knobs and flags the builder/config placeholders. It does not yet give each of these a standalone finding with the same precision as reflection, selection, acceptance, and population.

`gap`:

The absence of live batch, validation, merge, and stopping slots is not just "builder incomplete." These slots decide which evidence enters reflection, which held-out data remains hidden, how multi-parent lineage is recorded, and when the run stops. Without them, GEPA cannot preserve the train/validation/test semantics the user emphasized, and cannot reproduce GEPA+merge without ad hoc loop edits.

`correction`:

Add explicit findings or subsections for:

- `BatchSampler`: must sample from allowed train/search partition and return a typed no-cases error.
- `ValidationPolicy`: must request validation/admission work without feeding validation/test traces to reflective proposers by default.
- `MergeScheduler` / `GepaMerge`: must schedule merge as proposal work with pair causal provenance and surface-edit lowering.
- `Stopper`: must compose budget, iteration count, and optimizer state summary without mutating graph or hidden private state.

Evidence:

- `docs/specs/gepa_optimizer_surface.md:273-293` requires the builder to expose batch, validation, max metric calls, max iterations, seed, proposal count, and split policy.
- `docs/specs/gepa_optimizer_surface.md:320-341` includes batch sampling, validation policy, population observation, and iteration status in the step contract.
- `docs/specs/gepa_optimizer_surface.md:361-384` spells out train/search, validation, test, and probe behavior.
- `docs/specs/gepa_optimizer_surface.md:554-572` lists first product-grade GEPA implementation requirements.
- `docs/specs/gepa_optimizer_surface.md:580-587` defines default GEPA behavior, including minibatch and held-out validation/test semantics.
- `docs/specs/gepa_optimizer_surface.md:748-751` calls out trace attribution, validation hiding, test final-report-only, and shared reports as future milestones.
- `docs/specs/gepa_public_private_surface.md:753-765` says every load-bearing GEPA decision remains a trait slot, including validation and merge.
- `docs/specs/initial_library.md:3500-3529` documents GEPA surface-edit lowering and merge canonicalization.
- `reviews/2026-05-11-fuckery-extermination-today/surfaces/layer-2-gepa-customizer/README.md:47-58` lists the broader audit questions for merge, validation cadence, and batch sampler names.
- `reviews/2026-05-11-fuckery-extermination-today/surfaces/layer-2-gepa-customizer/agent-report-layer-2.md:653-668` summarizes missing live slots but does not break all of them out.

### GOV-008: Current Name Divergences Are Mostly Not Justified

`id`: GOV-008

`severity`: medium

`vision promise`:

The library should use conceptual names that map 1:1 to literature concepts. Naming is the index into the library's capabilities. The original nomenclature chose `ParentSelector`, `ParetoFrontier`, `Population`, `Acceptance`, `PreferenceRelation`, `Renderer`, `Materializer`, `Assessment`, and `Evidence` deliberately.

`current audit coverage`:

The audit catches several bad names or misleading names: `CandidateSelector` for GEPA parent selection, `Gate` for acceptance, `ReflectiveMutation` for a fixed edit, `WorstEvidencePart` for a placeholder, and renderer/materializer names exported without behavior.

`gap`:

The audit should classify divergences into three buckets:

1. justified Rust-native divergence from Python GEPA: typed artifact + `EditSurface` instead of `dict[str, str]`; renderer/materializer instead of `make_reflective_dataset`; `Assessment`/`Evidence` instead of scalar-only score;
2. acceptable lower-level/general name but wrong GEPA public name: `CandidateSelector`;
3. unjustified or false name: `Gate`, fixed `ReflectiveMutation`, `WorstEvidencePart`, placeholder renderer/materializer structs presented as standard pieces.

`correction`:

Add a "Nomenclature Refinement" section to `strategy-slots.md` or the integrated docs. For each name, state whether it is kept, renamed, or quarantined until behavior exists. The key hard cutovers are `CandidateSelector` to `ParentSelector` in GEPA-facing surfaces, `Gate` to `Acceptance`, fixed `ReflectiveMutation` to fixture naming, and placeholder evidence/rendering names out of ordinary public exports until implemented.

Evidence:

- `docs/specs/guiding_principles.md:63-67` makes literature-aligned names and predictable factoring core principles.
- `docs/specs/guiding_principles.md:339-343` says names should immediately suggest role and cites `ParentSelector` as good for GEPA parent choice.
- `docs/specs/initial_library.md:550-572` is the main nomenclature table.
- `docs/specs/gepa_optimizer_surface.md:102-132` justifies Rust-native replacement of Python GEPA's candidate maps.
- `docs/specs/gepa_optimizer_surface.md:773-779` explicitly rejects Python GEPA API compatibility, dict-shaped candidates, GEPA fields on artifacts, GEPA hooks in engine, and concrete provider deps in GEPA.
- `crates/leaven-gepa/src/selector.rs:34-40` uses `CandidateSelector`.
- `crates/leaven-gepa/src/gate.rs:23-27` uses `Gate`.
- `crates/leaven-gepa/src/proposer.rs:21-31` uses `ReflectiveMutation` for a fixed edit.
- `crates/leaven-gepa/src/part_selector.rs:72-74` uses `WorstEvidencePart` for a placeholder.
- `crates/leaven-render/src/lib.rs:10-15` exports renderer/materializer names.
- `crates/leaven-render/src/prompt.rs:1-3`, `crates/leaven-render/src/surface.rs:1-3`, `crates/leaven-render/src/run_graph.rs:1`, and `crates/leaven-render/src/materializer.rs:1-5` show those names have no behavior yet.

### GOV-009: Swappability Requires Public Strategy Contracts Plus Private State Discipline

`id`: GOV-009

`severity`: high

`vision promise`:

Swappability means changing one load-bearing GEPA strategy does not require forking the engine or reimplementing GEPA. Each slot may own private state, and private state affecting future decisions must participate in checkpoint/resume. The engine should provide capability-scoped context; strategies should not mutate graph internals or inspect forbidden splits.

`current audit coverage`:

The audit says current GEPA has strategy-shaped internals but not a real customizer API. It recommends request/context shapes for parent selection, part selection, feedback selection, mutation, acceptance, and population observation.

`gap`:

The audit underemphasizes private-state and checkpoint requirements. Swappability is not only "trait method exists"; it also requires deterministic/replayable state boundaries, no hidden globals, budget/cost accounting for side-effectful work, and no graph internals exposed as a workaround.

`correction`:

For each GEPA slot, specify:

- public customizer trait name and request type;
- allowed context/capabilities;
- private state that must checkpoint;
- event/cost/provenance output;
- must-not rules around graph mutation, hidden split reads, evaluator execution, and population updates.

The implementation plan should reject fixes that add `pub` graph internals or broad evidence-store handles to customizers. Give strategies scoped views or selected evidence/rendered payloads instead.

Evidence:

- `docs/specs/gepa_public_private_surface.md:506-519` requires small, swappable GEPA customizer traits and no engine fork/reimplementation.
- `docs/specs/gepa_public_private_surface.md:535` says every slot may own private state and relevant private state must participate in persistence.
- `docs/specs/gepa_optimizer_surface.md:546-549` requires checkpointed GEPA private state including RNG, sampler cursor, parent/part selector state, gate/admission state, merge scheduler state, and population.
- `docs/specs/guiding_principles.md:127-139` defines swappability as trait implementations, not engine patching.
- `docs/specs/guiding_principles.md:299-303` says engine decisions belong in strategy implementations, while the engine remains invariant.
- `docs/specs/guiding_principles.md:343-347` requires end-to-end observability of selected parents, proposals, evidence, acceptance, and frontier state.
- `docs/specs/gepa_optimizer_surface.md:531-540` requires reflection/merge/evaluator/cache/surface fingerprint cost and cache accounting discipline.
- `reviews/2026-05-11-fuckery-extermination-today/surfaces/layer-2-gepa-customizer/agent-report-layer-2.md:691-720` recommends request/context shapes but should add private-state and checkpoint/cost/event requirements.

## Refinement Edits Recommended

1. Update `reviews/2026-05-11-fuckery-extermination-today/refinement/vision-comparison.md` to add a GEPA-specific subsection: "GEPA is one optimizer, not the library." Fold in GOV-001 and explicitly route ownership across `leaven-run`, `leaven-gepa`, `leaven-engine`, and reusable vocabulary crates.

2. Update `reviews/2026-05-11-fuckery-extermination-today/surfaces/layer-2-gepa-customizer/strategy-slots.md` with a per-slot table covering parent selection, part selection, batch sampling, reflection/proposal, acceptance, validation, population/frontier, merge, and stopping. Columns should be Layer 1 visible surface, Layer 2 customizer API, lowered/private contract, current state, and correction.

3. Update `reviews/2026-05-11-fuckery-extermination-today/surfaces/layer-2-gepa-customizer/reflection-and-proposal.md` to distinguish deterministic Milestone A scaffolding from fake public reflection. Rename the correction target from "make `ReflectiveMutation` better" to "reserve `ReflectiveMutation` for async evidence/rendered-context reflection; quarantine fixed-edit fixtures."

4. Update `reviews/2026-05-11-fuckery-extermination-today/surfaces/layer-2-gepa-customizer/evidence-trace-selection.md` to state the single root violation: feedback and trace are present in evidence types but are collapsed before reflection. Add `FeedbackSelectionRequest` / renderer expectations as the correction surface.

5. Add or extend a Layer 2 subsection for batch, validation, merge, and stopping. These are currently listed but under-audited; each should have a finding or explicit non-finding.

6. Add a nomenclature refinement section, probably in `strategy-slots.md`, that classifies each divergence as justified Rust-native divergence, acceptable internal name but wrong public GEPA name, or unjustified production-looking placeholder.

7. Add a private-state discipline subsection to the integrated refinement docs: every swappable GEPA slot needs a request type, context/capability boundary, checkpoint state story, event/cost/provenance output, and must-not rules.
