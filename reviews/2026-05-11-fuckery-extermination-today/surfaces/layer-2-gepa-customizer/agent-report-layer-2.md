# Agent Report: Layer 2 GEPA Customizer Surface

Date: 2026-05-11

Scope audited:

- `crates/leaven-gepa`
- `crates/leaven-surface`
- `crates/leaven-population`
- `crates/leaven-preference`
- `crates/leaven-render`
- relevant GEPA/eval/product specs under `docs/specs`
- upstream GEPA/DSPy sources where useful for calibration

Question:

Can a GEPA power user swap strategy slots, reflection/proposal, trace/evidence
selection, parent selection, part selection, feedback selection, and acceptance
behavior without forking GEPA or losing necessary context?

Short answer: no. The current code has the beginning of a swappable value shape,
but it is not yet the Layer 2 customizer surface promised by the specs. A user
can swap some generic fields through `Gepa::with_strategies(...)`, and can
choose a surface, population, and fixed edit reflector through the builder. But
the live strategy contracts lose the context that real GEPA customization needs:
selected evidence, rendered feedback, trace payloads, selected part view,
lineage, objective/background, split policy, validation policy, merge policy,
and preference-aware acceptance.

The core failure pattern is not just missing code. The public names and specs
say "GEPA customizer slots"; the implementation provides narrower fixture-level
or scalar-only hooks that make a nearby example pass while leaving real GEPA
power users unable to implement the intended algorithms without forking or
reimplementing the GEPA loop.

## Governing Contract

The review tree contract requires findings to cite current paths and lines, to
separate public promises from implementation gaps, and to avoid treating
scaffolding as complete behavior:

- `reviews/2026-05-11-fuckery-extermination-today/AGENTS.md:8`
- `reviews/2026-05-11-fuckery-extermination-today/AGENTS.md:14`
- `reviews/2026-05-11-fuckery-extermination-today/AGENTS.md:25`

The user-message archive makes Layer 2 non-negotiable. The user explicitly
wanted a power-user surface for people building their own optimizers, with
swappable public/private contracts for GEPA parts:

- `reviews/2026-05-11-fuckery-extermination-today/complaints/session-user-messages-for-codex.md:176`
- `reviews/2026-05-11-fuckery-extermination-today/complaints/session-user-messages-for-codex.md:179`
- `reviews/2026-05-11-fuckery-extermination-today/complaints/session-user-messages-for-codex.md:191`

The specs agree:

- `docs/specs/gepa_public_private_surface.md:172` defines Layer 2 as "Customize
  GEPA".
- `docs/specs/gepa_public_private_surface.md:176` says customizers touch
  recognizable knobs: surface, parent selector, part selector, batch sampler,
  reflector/proposer, acceptance, population/frontier, validation cadence,
  merge, and stopping.
- `docs/specs/gepa_public_private_surface.md:506` says GEPA customizer traits
  must be small and swappable.
- `docs/specs/gepa_public_private_surface.md:518` says each trait must
  correspond to one load-bearing choice, and changing one must not require
  forking the engine or reimplementing GEPA.
- `docs/specs/guiding_principles.md:127` says every load-bearing decision in
  the loop must be a swappable trait implementation.

## Findings

### L2-001: Builder Advertises Layer 2 Strategy Swapping But Only Exposes A Small Subset

`id`: L2-001

`severity`: high

`surface`: Layer 2 GEPA customizer strategy slots

`evidence`:

- `docs/specs/gepa_optimizer_surface.md:73` shows the GEPA customizer path.
- `docs/specs/gepa_optimizer_surface.md:76` through
  `docs/specs/gepa_optimizer_surface.md:86` show builder calls for
  `.surface(...)`, `.parent_selector(...)`, `.part_selector(...)`,
  `.batch_sampler(...)`, `.reflector(...)`, `.acceptance(...)`,
  `.validation(...)`, `.population(...)`, and `.merge(...)`.
- `docs/specs/gepa_optimizer_surface.md:273` through
  `docs/specs/gepa_optimizer_surface.md:293` list the required builder
  controls.
- `crates/leaven-gepa/src/optimizer.rs:663` through
  `crates/leaven-gepa/src/optimizer.rs:668` implement only
  `GepaBuilder::surface`.
- `crates/leaven-gepa/src/optimizer.rs:677` through
  `crates/leaven-gepa/src/optimizer.rs:686` implement the default
  `.reflector(...)` path after a surface.
- `crates/leaven-gepa/src/optimizer.rs:688` through
  `crates/leaven-gepa/src/optimizer.rs:695` implement `.population(...)`.
- `crates/leaven-gepa/src/optimizer.rs:705` through
  `crates/leaven-gepa/src/optimizer.rs:713` implement `.reflector(...)`
  after population.
- `crates/leaven-gepa/src/optimizer.rs:716` through
  `crates/leaven-gepa/src/optimizer.rs:722` define `GepaConfig` and
  `MergeScheduler` as placeholders.

`promised behavior`:

Layer 2 users should be able to configure GEPA by swapping the public strategy
slots that correspond to GEPA algorithm choices. The public/private surface doc
names those slots directly:

- parent selector
- part selector
- batch sampler
- reflector/proposer
- acceptance
- population/frontier
- validation cadence
- merge
- stopper

`actual behavior`:

The public builder only exposes surface, population, and reflector. Parent
selector, part selector, and gate can be changed only by calling
`Gepa::with_strategies(...)` directly at
`crates/leaven-gepa/src/optimizer.rs:251`. Batch sampler, validation policy,
merge scheduling, and stopper/config are not implemented as live strategy slots.
`GepaConfig` and `MergeScheduler` are public names but carry no behavior.

`why it matters`:

This is exactly the Layer 2 gap the user warned about: "we don't ever want to
let go of that surface." A customizer can see the planned shape in the docs, but
cannot exercise it in code. They either drop into generic constructor
plumbing, reimplement the loop, or accept defaults they cannot replace. That
means Leaven currently has strategy-shaped internals, not a real GEPA
customizer API.

`correction direction`:

Hard cutover to a real Layer 2 builder. Add direct builder methods for every
implemented slot and remove or quarantine public placeholder names until their
behavior exists. If a slot is deliberately not in this milestone, the public
docs and exports should say so rather than presenting it as usable.

Target doc suggestion:

- `surfaces/layer-2-gepa-customizer/strategy-slots.md`

### L2-002: Reflection/Proposal Cannot See The Context A Real GEPA Reflector Needs

`id`: L2-002

`severity`: high

`surface`: reflection and proposal

`evidence`:

- `docs/specs/gepa_optimizer_surface.md:445` starts the Reflection and ASI
  contract.
- `docs/specs/gepa_optimizer_surface.md:450` through
  `docs/specs/gepa_optimizer_surface.md:458` specify standard reflective
  mutation as an LM-backed renderer/proposer.
- `docs/specs/gepa_optimizer_surface.md:460` through
  `docs/specs/gepa_optimizer_surface.md:471` say the renderer consumes parent
  candidate id, selected surface part, surface part view, assessment IDs,
  casewise evidence, optional attribution evidence, lineage summary, and
  objective/background.
- `crates/leaven-gepa/src/proposer.rs:6` through
  `crates/leaven-gepa/src/proposer.rs:18` define `SurfaceProposer` with only
  `artifact`, `surface`, and `part`.
- `crates/leaven-gepa/src/optimizer.rs:560` through
  `crates/leaven-gepa/src/optimizer.rs:563` call
  `reflector.propose_edit(&artifact, &self.surface, &part)`.
- `crates/leaven-engine/src/stage/proposer.rs:28` through
  `crates/leaven-engine/src/stage/proposer.rs:45` already define an async,
  graph-aware `Proposer<P>` trait.
- `crates/leaven-engine/src/context/proposal_context.rs:8` through
  `crates/leaven-engine/src/context/proposal_context.rs:12` hold graph,
  budget, and read scope for proposer stages.
- `crates/leaven-engine/src/context/run_context.rs:191` through
  `crates/leaven-engine/src/context/run_context.rs:208` route proposer calls
  through `RunContext::propose(...)` with proposal events and cost recording.

`promised behavior`:

A GEPA reflector should be able to read the selected parent, selected part,
part view, selected feedback, traces, lineage, objective/background, and budget
scope, then emit surface edits or native proposals with typed provenance.

`actual behavior`:

The live GEPA-local proposer trait cannot name assessments, evidence payloads,
feedback, trace refs, lineage, objective text, render context, materialized
context, budget, proposal count, or native proposal output. It is also
synchronous. The GEPA loop bypasses the engine proposer stage and manually
records a proposal batch afterward.

`why it matters`:

Real GEPA reflection is not "given an artifact and a part, return a canned
edit." It is "read evidence and traces about why the parent failed, then
propose a targeted mutation." Upstream GEPA and DSPy both expose richer
feedback paths: Python GEPA `ReflectionConfig` includes a batch sampler,
module selector, reflection LM, prompt template, and custom proposer at
`/Users/darin/vendor/github.com/gepa-ai/gepa/src/gepa/optimize_anything.py:716`,
and DSPy GEPA metrics receive full trace, predictor name, and predictor trace at
`/Users/darin/vendor/github.com/stanfordnlp/dspy/dspy/teleprompt/gepa/gepa.py:27`.
Leaven's current trait is too narrow for that class of customizer.

`correction direction`:

Replace `SurfaceProposer` as the product GEPA reflection contract. Either route
GEPA reflection through engine `Proposer<P>` with a typed `GepaMutationRequest`,
or create an equally honest GEPA-local async trait that receives a request plus
scoped proposal/render/evidence context. The request must include the selected
candidate, selected part, selected part view or renderable handle, feedback
assessment IDs, selected evidence/rendered feedback, lineage/context, and
proposal count. Surface edit output and native proposal output should both be
legal.

Target doc suggestion:

- `surfaces/layer-2-gepa-customizer/reflection-and-proposal.md`

### L2-003: Feedback And Trace Evidence Are Captured In Types But Dropped Before Reflection

`id`: L2-003

`severity`: high

`surface`: trace/evidence selection and feedback selection

`evidence`:

- `docs/specs/gepa_optimizer_surface.md:475` through
  `docs/specs/gepa_optimizer_surface.md:483` list ASI/feedback sources:
  evaluator evidence fields, casewise scalar outcomes, attribution evidence,
  command/stdout/stderr evidence, transcript refs, validation/apply errors, and
  previous candidate summaries.
- `docs/specs/guiding_principles.md:321` through
  `docs/specs/guiding_principles.md:323` say trace is opaque and rendering is
  the bridge.
- `crates/leaven-evidence/src/feedback.rs:8` through
  `crates/leaven-evidence/src/feedback.rs:14` define
  `ScoredFeedbackEvidence` with score, feedback, and trace.
- `crates/leaven-evidence/src/feedback.rs:33` through
  `crates/leaven-evidence/src/feedback.rs:42` expose feedback and trace
  accessors.
- `crates/leaven-gepa/src/optimizer.rs:57` through
  `crates/leaven-gepa/src/optimizer.rs:65` implement
  `GepaScoreEvidence` for `CasewiseEvidence<ScoredFeedbackEvidence>` by
  projecting only scalar scores.
- `crates/leaven-gepa/src/optimizer.rs:625` through
  `crates/leaven-gepa/src/optimizer.rs:635` retrieve evidence, project scalar
  casewise evidence, and compute an average score.
- `crates/leaven-gepa/src/proposer.rs:40` through
  `crates/leaven-gepa/src/proposer.rs:46` show the current proposer receives no
  evidence or feedback.

`promised behavior`:

GEPA customizers should be able to decide what feedback and traces the
reflector consumes, and trace/evidence rendering should be an explicit,
swappable bridge.

`actual behavior`:

GEPA can evaluate a candidate and retrieve typed evidence internally, but the
only evidence shape that reaches population/gate logic is scalar casewise
evidence. Feedback strings and trace lines are not passed into the reflector.
There is no feedback selector and no reflection renderer in the live GEPA loop.

`why it matters`:

This breaks the core GEPA product idea. A power user who returns natural
language feedback from scoring cannot get that feedback to the proposal stage
without forking or encoding it into the artifact/surface out of band. That also
means examples can show score movement without proving that Leaven can do
feedback-driven reflection.

`correction direction`:

Introduce a GEPA feedback selection/rendering stage between evaluation and
reflection. It should preserve assessment IDs and evidence refs, choose which
case outcomes or traces are shown, render them through `leaven-render`, and pass
that rendered feedback into the reflector/proposer request. Do not collapse
`ScoredFeedbackEvidence` to `ScalarEvidence` before reflection.

Target doc suggestion:

- `surfaces/layer-2-gepa-customizer/evidence-trace-selection.md`

### L2-004: Trace-Aware Part Selection Is Promised But The Live Part Selector Cannot See Evidence

`id`: L2-004

`severity`: high

`surface`: part selection

`evidence`:

- `docs/specs/gepa_public_private_surface.md:248` through
  `docs/specs/gepa_public_private_surface.md:276` explain the load-bearing
  parent/part distinction.
- `docs/specs/gepa_public_private_surface.md:526` says `PartSelector` input is
  selected artifact, surface, and optional attributed evidence.
- `docs/specs/initial_library.md:1370` through
  `docs/specs/initial_library.md:1380` distinguish casewise measurement from
  attribution and say trace-aware selectors consume attribution.
- `docs/specs/initial_library.md:3476` through
  `docs/specs/initial_library.md:3500` describe `RoundRobinPart` and
  `InvokedAndFailingPart`, with `InvokedAndFailingPart` consuming
  `AttributableEvidence<S::PartId>`.
- `crates/leaven-gepa/src/part_selector.rs:6` through
  `crates/leaven-gepa/src/part_selector.rs:13` define `PartSelector` with only
  artifact and surface.
- `crates/leaven-gepa/src/part_selector.rs:72` through
  `crates/leaven-gepa/src/part_selector.rs:74` define `WorstEvidencePart` as a
  placeholder name.
- `crates/leaven-evidence/src/lib.rs:11` through
  `crates/leaven-evidence/src/lib.rs:17` define an `AttributableEvidence<K>`
  trait, but GEPA part selection does not consume it.

`promised behavior`:

Part selectors should be swappable, and trace-aware selectors should be able to
choose parts based on attributed failing evidence or trace relevance.

`actual behavior`:

The live `PartSelector` trait can list parts from the artifact through the
surface, but cannot inspect evidence, case outcomes, attribution, trace refs,
split role, selected parent history, or current feedback batch. The only real
standard implementation is round-robin. `WorstEvidencePart` is public
scaffolding with no behavior.

`why it matters`:

Part selection is the "where do I edit?" choice. For agentic and multi-part
artifacts, the right part often comes from trace attribution: failed tool call,
bad skill file, broken prompt section, or module-specific feedback. Without
that evidence, Leaven cannot implement the trace-aware GEPA variants the specs
explicitly call out.

`correction direction`:

Change the part-selection request shape to include the selected parent id,
artifact, surface, selected minibatch/assessment IDs, and optional attribution
or rendered feedback. `RoundRobinPart` can ignore this extra context, but
trace-aware selectors must receive it. Rename or remove `WorstEvidencePart`
until it is implemented honestly.

Target doc suggestion:

- `surfaces/layer-2-gepa-customizer/evidence-trace-selection.md`
- `surfaces/layer-2-gepa-customizer/strategy-slots.md`

### L2-005: Parent Selection Is Named As A Candidate Selector And The Default Is Not Frequency Weighted

`id`: L2-005

`severity`: medium

`surface`: parent selection

`evidence`:

- `docs/specs/eval_nomenclature.md:63` through
  `docs/specs/eval_nomenclature.md:89` say Layer 2 should use
  `parent_selector` and reserve `candidate_selector` for lower-level internals.
- `docs/specs/initial_library.md:3341` through
  `docs/specs/initial_library.md:3352` define GEPA components, including
  `ParentSelector`.
- `docs/specs/initial_library.md:3367` through
  `docs/specs/initial_library.md:3377` specify a richer
  `ParentSelector::select` with population view, graph view, selection context,
  selection error, and selection outcome observation.
- `crates/leaven-gepa/src/selector.rs:34` through
  `crates/leaven-gepa/src/selector.rs:40` define `CandidateSelector`.
- `crates/leaven-gepa/src/selector.rs:79` through
  `crates/leaven-gepa/src/selector.rs:85` name `ParetoFrequencyWeighted`.
- `crates/leaven-gepa/src/selector.rs:95` through
  `crates/leaven-gepa/src/selector.rs:104` implement
  `ParetoFrequencyWeighted` by returning `population.best_candidate()`.
- `crates/leaven-gepa/tests/gepa_smoke.rs:283` through
  `crates/leaven-gepa/tests/gepa_smoke.rs:313` assert that the selectors
  delegate to population best candidate.

`promised behavior`:

GEPA-facing APIs should call this a parent selector, and
`ParetoFrequencyWeighted` should express paper-style instance-Pareto frequency
selection over frontier members.

`actual behavior`:

The trait is named `CandidateSelector`, not `ParentSelector`. The default named
`ParetoFrequencyWeighted` is deterministic best-candidate selection and ignores
the graph view. It has no RNG, no frequency weighting, no outcome observation,
and no typed "no parent" decision.

`why it matters`:

The name mismatch is not just cosmetic. The public concept is "which parent do
we mutate next?" The current trait and default implementation make the GEPA
customizer surface feel generic and underpowered. Worse, a user choosing
`ParetoFrequencyWeighted` would reasonably believe they are reproducing the
GEPA paper's parent selection, but the implementation is a best-candidate
proxy.

`correction direction`:

Hard cutover the GEPA-facing trait and builder slot to `ParentSelector`.
Implement real frequency-weighted frontier sampling or rename the current
implementation to `SelectBestParent` / `SelectBestCandidate` until the paper
strategy exists. Add selection context, selection errors, and optional outcome
observation before treating it as a durable Layer 2 slot.

Target doc suggestion:

- `surfaces/layer-2-gepa-customizer/strategy-slots.md`

### L2-006: Acceptance Is A Scalar Gate, Not A Swappable Evidence/Preference Policy

`id`: L2-006

`severity`: high

`surface`: acceptance behavior

`evidence`:

- `docs/specs/gepa_public_private_surface.md:523` through
  `docs/specs/gepa_public_private_surface.md:530` say `Acceptance` receives
  parent/child comparable score summaries and configured metric axes, and must
  not update population or request hidden test evidence.
- `docs/specs/gepa_optimizer_surface.md:568` says GEPA must accept or reject
  children through a swappable `Acceptance`.
- `docs/specs/initial_library.md:1382` through
  `docs/specs/initial_library.md:1395` define `PreferenceRelation` as the
  concept that interprets evidence.
- `docs/specs/initial_library.md:1421` through
  `docs/specs/initial_library.md:1445` include `Incomparable` and say scores
  are one evidence shape plus one preference relation.
- `crates/leaven-gepa/src/gate.rs:23` through
  `crates/leaven-gepa/src/gate.rs:26` define `Gate` as `fn decide(&mut self,
  parent_score: f64, candidate_score: f64)`.
- `crates/leaven-gepa/src/optimizer.rs:403` through
  `crates/leaven-gepa/src/optimizer.rs:409` evaluate the child and decide from
  parent and child average scores.
- `crates/leaven-preference/src/pareto.rs:1` defines only an empty
  `ParetoPreference`.
- `crates/leaven-preference/src/ranking.rs:1` through
  `crates/leaven-preference/src/ranking.rs:5` define empty ranking preference
  marker structs.

`promised behavior`:

Acceptance is a GEPA strategy slot that decides whether a child deserves
validation/admission by interpreting evidence under a preference relation and
split policy.

`actual behavior`:

Acceptance is currently a scalar gate over two `f64` averages. The GEPA loop
cannot pass evidence refs, per-case outcomes, metric axes, pairwise/listwise
evidence, trace claims, validation policy, or decision reasons into
acceptance. The `leaven-preference` crate does not provide live preference
relations for GEPA to use.

`why it matters`:

This prevents customizers from implementing pairwise acceptance, listwise
acceptance, multi-axis Pareto acceptance, validation-aware admission,
claim-aware acceptance, or "defer" decisions. Upstream GEPA's acceptance
protocol receives the full candidate proposal and GEPA state, including
before/after subsample evaluations, trajectories, parent ids, metadata,
validation scores, Pareto frontier, and iteration count
(`/Users/darin/vendor/github.com/gepa-ai/gepa/src/gepa/strategies/acceptance.py:10`).
Leaven does not need to copy that Python shape, but the Rust slot must be
equally honest about context.

`correction direction`:

Replace public GEPA `Gate` with `Acceptance` over a typed request:
parent candidate id, child candidate id, screening assessment IDs, comparable
evidence summaries, split/purpose, optional validation state, and graph view.
Return a decision with reason, not just a bool-like enum. Keep simple
`StrictImprovement` as one implementation over scalar summaries, not as the
shape of the whole contract.

Target doc suggestion:

- `surfaces/layer-2-gepa-customizer/strategy-slots.md`
- `cross-cutting/preference-population.md` if a cross-cutting preference report
  is added

### L2-007: GEPA Population Is Scalar Casewise Only, So Rich Population Strategies Cannot Plug In

`id`: L2-007

`severity`: high

`surface`: population/frontier strategy and evidence neutrality

`evidence`:

- `docs/specs/guiding_principles.md:114` through
  `docs/specs/guiding_principles.md:125` require evidence-shape neutrality and
  reject faking pairwise evidence through scalar scores.
- `docs/specs/guiding_principles.md:127` through
  `docs/specs/guiding_principles.md:139` require no-frontier, tournament, and
  user-defined frontier configurations without forking.
- `docs/specs/initial_library.md:1447` through
  `docs/specs/initial_library.md:1451` define population as live optimizer
  state, not the policy for choosing what to try next.
- `crates/leaven-gepa/src/optimizer.rs:68` through
  `crates/leaven-gepa/src/optimizer.rs:81` define `GepaPopulation` as
  observing only `CasewiseEvidence<ScalarEvidence>`.
- `crates/leaven-population/src/tournament.rs:78` through
  `crates/leaven-population/src/tournament.rs:146` implement a
  `TournamentPopulation` with pairwise Bradley-Terry-style state.
- `crates/leaven-population/src/no_population.rs:1` defines `NoPopulation`,
  but it has no GEPA integration.

`promised behavior`:

Population/frontier should be a swappable strategy. Scalar Pareto, no
frontier, tournament, MAP-Elites, novelty, and custom populations should be
expressible without changing the GEPA loop.

`actual behavior`:

GEPA population observation is hard-wired to scalar casewise evidence. `KeepBest`
and `ParetoFrontier` are adapted into `GepaPopulation`; pairwise tournament
population and no-population modes cannot participate in GEPA's population slot
without a new adapter or loop change.

`why it matters`:

The population crate contains richer strategy names, but GEPA cannot use them
as GEPA populations. This collapses Leaven back toward a scalar-only GEPA loop,
contradicting the spec's evidence-shape neutrality and power-user goals.

`correction direction`:

Split "what evidence did this assessment produce?" from "how should this
population observe it?" A population strategy should receive candidate ids,
assessment ids, scoped graph view or evidence access, and split/purpose context,
then decide whether/how to update. Scalar casewise observation should be one
implementation path, not the trait signature.

Target doc suggestion:

- `surfaces/layer-2-gepa-customizer/strategy-slots.md`
- `cross-cutting/preference-population.md` if a cross-cutting preference report
  is added

### L2-008: Renderers Needed For Reflection Are Public Names With No Behavior

`id`: L2-008

`severity`: medium

`surface`: rendering and materialization for feedback context

`evidence`:

- `docs/specs/gepa_public_private_surface.md:302` says feedback/traces reach
  the reflector through renderers.
- `docs/specs/gepa_optimizer_surface.md:473` says reflection renderer output is
  provider-neutral LM input through `leaven-lm`.
- `docs/specs/guiding_principles.md:321` through
  `docs/specs/guiding_principles.md:323` say rendering is the bridge from
  opaque traces/evidence to consumers.
- `crates/leaven-render/src/lib.rs:10` through
  `crates/leaven-render/src/lib.rs:15` publicly export renderer/materializer
  names.
- `crates/leaven-render/src/prompt.rs:1` through
  `crates/leaven-render/src/prompt.rs:3` define only empty
  `ReflectionPromptRenderer` and `StructuredPromptRenderer` structs.
- `crates/leaven-render/src/surface.rs:1` through
  `crates/leaven-render/src/surface.rs:3` define only empty surface renderer
  structs.
- `crates/leaven-render/src/run_graph.rs:1` defines only an empty
  `RunGraphDebugRenderer`.
- `crates/leaven-render/src/materializer.rs:1` through
  `crates/leaven-render/src/materializer.rs:4` define only empty materializer
  structs.

`promised behavior`:

Reflection should not inspect arbitrary trace/evidence internals directly.
Renderers/materializers should provide the bridge from typed graph/evidence
truth to reflection prompts, structured LM messages, or workspace layouts.

`actual behavior`:

The renderer names exist and are public, but have no behavior. GEPA does not
depend on them in its reflection path despite `leaven-gepa` depending on
`leaven-render` in `crates/leaven-gepa/Cargo.toml:21`.

`why it matters`:

Even if the reflector trait were fixed, there is no standard rendering path for
the selected feedback/trace context. This encourages customizers to build
private string formatting and repeat the Python string-map failure mode the
specs are trying to avoid.

`correction direction`:

Either implement the minimum reflection rendering path or stop exporting these
names as product-ready surface. The first useful cut should render selected
casewise feedback, selected part view, lineage summary, and optional trace refs
into provider-neutral `leaven-lm` input.

Target doc suggestion:

- `surfaces/layer-2-gepa-customizer/evidence-trace-selection.md`
- `cross-cutting/rendering-scaffolding.md` if a cross-cutting rendering report
  is added

## Non-Findings

### NF-L2-001: `leaven-surface` Has The Right Core Shape For Layer 2

I do not see `leaven-surface` as the blocker for this layer.

Evidence:

- `crates/leaven-surface/src/edit_surface.rs:42` defines `EditSurface<A>`.
- `crates/leaven-surface/src/edit_surface.rs:46` through
  `crates/leaven-surface/src/edit_surface.rs:73` keep `PartId`, `Address`,
  `View<'a>`, and `Edit` in the surface type system.
- `crates/leaven-surface/src/edit_surface.rs:96` through
  `crates/leaven-surface/src/edit_surface.rs:116` define pure
  `change_part(...)` lowering into artifact-native changes.
- `crates/leaven-surface/src/lib.rs:31` through
  `crates/leaven-surface/src/lib.rs:47` document the surface laws.

Why this is acceptable:

The surface crate preserves the Rust-native replacement for Python GEPA's
`dict[str, str]` candidate map: typed artifact, typed surface, typed part id,
borrowed part view, surface-native edit, and artifact-native change. The issue
is downstream: GEPA does not pass enough selected part view, evidence,
rendering, or context into the reflector and selectors.

## Cross-Cutting Root Cause

The implementation currently treats GEPA as a reusable loop over scalar
casewise evidence plus one surface edit. That is a useful scaffold, but Layer 2
requires GEPA to be a configured composition of strategy values. The current
composition is incomplete in two ways:

1. Several slots are not live: batch sampler, validation policy, merge, stopper,
   config, feedback selector, renderer.
2. Several live slots have signatures that discard necessary context:
   reflector, part selector, acceptance, population observation.

The safest correction is not backward-compatible layering around the current
fixtures. It is a hard cutover from fixture names to honest strategy contracts.
Where a placeholder remains, name it as scaffolding or remove it from the
ordinary public path.

## Suggested Target Documents

Use these destinations if the audit is split into the planned layer files:

- `surfaces/layer-2-gepa-customizer/strategy-slots.md`
  - L2-001
  - L2-005
  - L2-006
  - L2-007
- `surfaces/layer-2-gepa-customizer/reflection-and-proposal.md`
  - L2-002
- `surfaces/layer-2-gepa-customizer/evidence-trace-selection.md`
  - L2-003
  - L2-004
  - L2-008
- `cross-cutting/rendering-scaffolding.md`
  - L2-008, if cross-cutting rendering placeholders are inventoried separately
- `cross-cutting/preference-population.md`
  - L2-006 and L2-007, if preference/population neutrality is inventoried
    separately

## Implementation Guidance For Future Fix Work

A future implementor should not try to "complete" Layer 2 by adding more
parameters to the current fixture traits. The missing concept is a request
shape per strategy slot.

Minimum request/context shapes to design:

- `ParentSelectionRequest`: population view/state, scoped graph view,
  selection RNG/context, optional split/search state, and previous selection
  outcome hook.
- `PartSelectionRequest`: selected parent id, artifact, surface, part list,
  selected assessment ids, optional attribution evidence, and rendered feedback
  handles.
- `FeedbackSelectionRequest`: parent/child candidates, assessment ids,
  casewise outcomes, trace/evidence refs, split purpose, budget/render limits,
  and policy for hidden validation/test data.
- `GepaMutationRequest`: parent id, selected part id, part view or renderable
  handle, rendered feedback, lineage summary, objective/background, requested
  proposal count, and allowed output shape.
- `AcceptanceRequest`: parent id, child id, before/after screening
  assessments, comparable summaries, evidence refs, preference relation,
  split/purpose, and validation/admission context.
- `PopulationObservationRequest`: candidate id, assessment ids, split/purpose,
  evidence refs, scoped graph view, and population-specific interpretation
  policy.

The simple defaults can ignore much of this context. The public contracts
cannot. If the context is not in the request, a power user cannot implement the
strategy without forking GEPA.
