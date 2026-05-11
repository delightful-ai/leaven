# Layer 2 GEPA Root Cause Map

Status: canonical Layer 2 audit document.

Scope: GEPA customizer failures only. This is not a broad Leaven root-cause
map, not a Layer 1 product-builder report, and not a Layer 3 engine-author
report.

## Governing Shape

Ideal contract: GEPA is one optimizer value made from swappable GEPA-specific
strategies. It is not the engine and not the whole library. The initial spec
says Leaven supports GEPA-style reflective prompt evolution while also
supporting other optimizer families, and explicitly rejects cold-core
assumptions such as scalar-only evaluation, Pareto-only selection, one-shot LM
proposals, train/validation as universal, and every optimizer having GEPA's loop
shape (`docs/specs/initial_library.md:406-423`). The same section defines GEPA
as one optimizer composed from parent selector, part selector, batch sampler,
reflector/proposer, acceptance policy, validation policy, population/frontier,
and optional merge proposer (`docs/specs/initial_library.md:443`).

Current reality: `Gepa` is a reusable loop scaffold, but the live customizer
surface is narrower than the contract. The struct owns `surface`, `population`,
`reflector`, `parent_selector`, `part_selector`, and `gate`, plus a hard-coded
`TRAIN` partition and iteration state (`crates/leaven-gepa/src/optimizer.rs:176-197`).
The public builder exposes only `surface`, `population`, and `reflector`
(`crates/leaven-gepa/src/optimizer.rs:659-714`), while public placeholder names
for config and merge carry no behavior (`crates/leaven-gepa/src/optimizer.rs:716-722`).

User impact: Layer 2 users see GEPA-shaped names, but they cannot swap the
load-bearing algorithm decisions promised by the specs without dropping into
generic constructor plumbing, accepting scalar/fixed defaults, or forking the
loop.

## RC-L2-001: Milestone Scaffolding Escaped As Product Reflection

Ideal contract: Milestone A permits a deterministic proposer only to prove the
real GEPA loop; Milestone B is where `ReflectiveMutation` must consume LM
vocabulary, mock LM output, evidence rendering, and typed proposer feedback
(`docs/specs/gepa_optimizer_surface.md:692-713`). Standard reflective mutation
is defined as renderer/proposer behavior over parent candidate id, selected
part, part view, assessment IDs, evidence, attribution, lineage, and
objective/background (`docs/specs/gepa_optimizer_surface.md:445-483`).

Current implementation: the live public `ReflectiveMutation<E>` stores one edit
and always returns it while ignoring artifact, surface, and part
(`crates/leaven-gepa/src/proposer.rs:21-47`). It is re-exported from
`leaven-gepa` and its prelude (`crates/leaven-gepa/src/lib.rs:18-33`). The AIME
example uses it as the GEPA reflector and hard-codes the optimized system prompt
(`examples/p8_aime_gepa/src/main.rs:81-94`).

Blocker/gap: this is not a missing polish issue. A fixture has the public name
of the real reflective stage, so examples can show GEPA-like score movement
without proving feedback-driven reflection.

Correction direction: hard cutover the fixture to an honest name such as
`FixedEditProposer` or move it to tests/examples. Reserve `ReflectiveMutation`
for async evidence/rendered-context reflection. Do not keep both production
names in ordinary public paths.

Required proof/tests: a GEPA smoke test may keep a fixed-edit fixture only under
fixture naming. Product proof must include a mock-LM reflective mutation that
reads selected feedback and part view before producing an edit, plus an example
asserting the reflector input contains nonempty feedback/trace selected from an
assessment.

## RC-L2-002: Strategy Slots Are Fields, Not Public Contracts

Ideal contract: Layer 2 users should touch recognizable GEPA knobs: surface,
parent selector, part selector, batch sampler, reflector/proposer, acceptance,
population/frontier, validation cadence, merge, and stopping
(`docs/specs/gepa_public_private_surface.md:172-189`). The optimizer surface
requires the builder to support those knobs plus max metric calls, iterations,
seed, proposal count, history tracking, and split policy
(`docs/specs/gepa_optimizer_surface.md:273-293`).

Current implementation: `Gepa::with_strategies` accepts only surface,
population, reflector, parent selector, part selector, and gate
(`crates/leaven-gepa/src/optimizer.rs:246-272`). The public builder narrows that
further to surface, optional population, and reflector
(`crates/leaven-gepa/src/optimizer.rs:663-713`). `ValidationPolicy` is a marker
trait with empty `FullValidation` and `MinibatchThenValidation` markers
(`crates/leaven-gepa/src/validation.rs:1-16`). There is no live batch sampler,
merge scheduler behavior, or stopper contract in `leaven-gepa`.

Blocker/gap: the slot map exists in specs and partial fields, but there is no
request/output/error/state contract per slot. A future implementor could add
builder methods that simply forward to the current narrow traits and still miss
the real contract.

Correction direction: define public Layer 2 slot traits and request/response
types before expanding the builder. Builder methods must map directly to those
contracts, not to placeholders or scalar-only helpers.

Required proof/tests: compile-fail or API contract tests should prove the
builder exposes every required slot and rejects incomplete/contradictory
configurations before a run starts, including no surface, no reflector/default
reflection path, empty required train/search partition, invalid validation
policy, and invalid merge configuration (`docs/specs/gepa_optimizer_surface.md:295-304`).

## RC-L2-003: Reflection Context Is Lost Before The Reflector

Ideal contract: a GEPA reflection renderer consumes parent candidate id,
selected surface part, surface part view, screening/minibatch assessment IDs,
casewise evidence, optional attribution evidence, lineage summary, and
objective/background, then produces provider-neutral LM input
(`docs/specs/gepa_optimizer_surface.md:460-473`). Feedback sources include
evaluator evidence fields, casewise outcomes, attribution, command output,
transcript refs, validation/apply errors, and previous candidate summaries
(`docs/specs/gepa_optimizer_surface.md:475-483`).

Current implementation: `SurfaceProposer` receives only `artifact`, `surface`,
and `part` (`crates/leaven-gepa/src/proposer.rs:6-18`). The GEPA loop calls
`propose_edit(&artifact, &self.surface, &part)` before lowering and recording a
proposal batch itself (`crates/leaven-gepa/src/optimizer.rs:549-593`). It passes
neither part view nor assessments/evidence/trace/objective into the reflector.

Blocker/gap: real GEPA reflection cannot be implemented through this trait. A
customizer must smuggle context through artifact/surface state or fork the
optimizer.

Correction direction: replace the product reflection surface with an async
`GepaProposer` or `ReflectiveMutation` over a `GepaMutationRequest` plus scoped
proposal/render/evidence context. The request must name parent, selected part,
part view or render handle, selected assessments/evidence refs, proposal count,
and output mode.

Required proof/tests: a reflector contract test should fail if a mock reflector
does not receive the parent id, selected part, part view, assessment id, feedback
text, trace excerpt, and `informed_by` refs needed to produce a proposal.

## RC-L2-004: Evidence Is Collapsed To Scalar Before Strategy Interpretation

Ideal contract: evidence shape and preference are separate. The initial spec
says casewise measurement and attribution are different contracts
(`docs/specs/initial_library.md:1370-1380`), `PreferenceRelation` interprets
evidence (`docs/specs/initial_library.md:1382-1445`), and population is live
optimizer state rather than parent-selection policy (`docs/specs/initial_library.md:1447-1451`).
The guiding principles require evidence-shape neutrality and reject faking
pairwise evidence through scalar scores (`docs/specs/guiding_principles.md:114-125`).

Current implementation: `ScoredFeedbackEvidence` preserves score, natural
language feedback, and trace (`crates/leaven-evidence/src/feedback.rs:8-43`),
but `GepaScoreEvidence for CasewiseEvidence<ScoredFeedbackEvidence>` projects it
to `CasewiseEvidence<ScalarEvidence>` by keeping only scores
(`crates/leaven-gepa/src/optimizer.rs:57-65`). `Gate` accepts only two `f64`
scores (`crates/leaven-gepa/src/gate.rs:23-27`). `GepaPopulation` observes only
`CasewiseEvidence<ScalarEvidence>` (`crates/leaven-gepa/src/optimizer.rs:68-81`).

Blocker/gap: the current loop drops the feedback and trace before the slots that
need to select feedback, choose parts, reflect, accept, or update population. It
also makes pairwise/listwise/multi-axis GEPA variants impossible without loop
changes.

Correction direction: preserve assessment IDs, evidence refs, and selected
payload/rendered views until strategy slots interpret them. Scalar strict
improvement remains one default implementation, not the shape of `Acceptance`,
`PopulationObservation`, or reflection.

Required proof/tests: add non-scalar GEPA-adjacent contract tests: feedback
evidence reaches a reflector, attribution can drive part selection, and a
population/acceptance implementation can consume evidence without converting to
average `f64`.

## RC-L2-005: GEPA Bypasses Engine Proposal Finalization Semantics

Ideal contract: proposers are async stages with typed request shapes and
`ProposalContext`; optimizers should use `ctx.propose(&proposer, request)` when
possible so stage events and costs are recorded uniformly
(`docs/specs/initial_library.md:2174-2233`). The live engine proposer trait is
async and receives `ProposalContext` (`crates/leaven-engine/src/stage/proposer.rs:27-46`).
`ProposalContext` exposes graph, read scope, budget, render context, and
materialize context (`crates/leaven-engine/src/context/proposal_context.rs:8-62`).
`RunContext::propose` finalizes proposer calls through stage events and cost
recording (`crates/leaven-engine/src/context/run_context.rs:191-208`).

Current implementation: GEPA calls the local synchronous proposer, lowers the
edit, then directly calls `record_proposal_batch` and `apply_batch`
(`crates/leaven-gepa/src/optimizer.rs:560-593`). It charges a hard-coded
`Cost::metric_calls(1)` for proposal recording (`crates/leaven-gepa/src/optimizer.rs:570-586`).

Blocker/gap: GEPA has a second proposal path. This loses the product guarantee
that proposal generation is budgeted, observable, scoped, and finalizable in one
engine-owned way.

Correction direction: GEPA may own GEPA-specific request/output adapters, but
proposal generation must flow through `RunContext::propose` or one equivalent
engine finalizer that preserves cost, events, read scope, render/materialize
context, proposal provenance, and graph mutation authority.

Required proof/tests: assert GEPA reflection emits the same proposer-stage
events and budget charges as a direct engine proposer call, and that a budget
exhaustion in proposal generation fails before graph mutation as required by the
GEPA optimizer spec (`docs/specs/gepa_optimizer_surface.md:528-533`).

## RC-L2-006: Naming Divergence Hides Missing Behavior

Ideal contract: names are infrastructure. The initial spec maps parent
selection to `ParentSelector`, acceptance/admission to `Acceptance`,
opaque-to-visible bridges to `Renderer`/`Materializer`, and explicitly avoids
`CandidateSelector` in GEPA-facing APIs and `Gate` for the public admission slot
(`docs/specs/initial_library.md:531-572`). The public/private spec separately
explains parent selection versus part selection and reserves `candidate_selector`
for lower-level internal use (`docs/specs/gepa_public_private_surface.md:246-287`).

Current implementation: public exports include `CandidateSelector`, `Gate`,
fixed `ReflectiveMutation`, `WorstEvidencePart`, `GepaConfig`, `MergeScheduler`,
`ReflectiveMutationConfig`, and `SystemAwareMerge`
(`crates/leaven-gepa/src/lib.rs:10-25`). `ParetoFrequencyWeighted` is documented
as current deterministic best-candidate selection, not stochastic
frequency-weighted sampling (`crates/leaven-gepa/src/selector.rs:79-104`).
`WorstEvidencePart` is only a placeholder name (`crates/leaven-gepa/src/part_selector.rs:72-74`).
Renderer/materializer names are publicly exported while the structs are empty
(`crates/leaven-render/src/lib.rs:10-15`,
`crates/leaven-render/src/prompt.rs:1-3`,
`crates/leaven-render/src/surface.rs:1-3`,
`crates/leaven-render/src/run_graph.rs:1`,
`crates/leaven-render/src/materializer.rs:1-5`).

Blocker/gap: these names tell a future implementor the concepts already exist.
Some are acceptable internal or future names, but not public Layer 2 evidence of
working behavior.

Correction direction: hard cut public names to match behavior. `CandidateSelector`
becomes public GEPA `ParentSelector`; scalar `Gate` becomes one internal helper
or default implementation under `Acceptance`; `ParetoFrequencyWeighted` is
implemented or renamed; placeholders are removed from ordinary public exports or
kept under explicit scaffold/fixture naming.

Required proof/tests: add an export/stub guard that fails on approved-for-future
placeholder structs in ordinary preludes unless they carry a real behavior test
or live under a clearly named fixture/scaffold module.

## RC-L2-007: Private State Exists For Some Slots But Not The Slot Set

Ideal contract: every GEPA slot may own private state, and private state that
affects future decisions must be derivable from graph truth or included in the
optimizer checkpoint schema (`docs/specs/gepa_public_private_surface.md:535-537`).
The GEPA optimizer spec requires checkpointed private state for RNG, batch
sampler cursor, parent/part selector state, gate/admission state, merge
scheduler state, and population state when not derivable from graph events
(`docs/specs/gepa_optimizer_surface.md:543-550`).

Current implementation: `GepaCheckpointState` includes train partition, max and
completed iterations, best, observed, population, parent selector, part selector,
and gate (`crates/leaven-gepa/src/optimizer.rs:199-211`). It does not include
RNG, batch sampler, validation policy, merge scheduler, stopper/config, or
reflection state because those live slots do not exist. Tests prove checkpoint
round-trips for selector cursor and population frontier membership
(`crates/leaven-gepa/tests/gepa_smoke.rs:180-263`).

Blocker/gap: checkpoint discipline is partially real, but only for the reduced
slot set. Adding slots later without checkpoint contracts would make resumable
GEPA nondeterministic or silently wrong.

Correction direction: each slot contract must include private state and
checkpoint behavior before being public. Stateless defaults can use `()` state,
but stateful samplers, RNG-based selectors, validation cadence, merge schedules,
and reflectors with cache/cursor state must snapshot explicitly.

Required proof/tests: for every stateful slot, add a resume test proving that a
checkpoint taken mid-run resumes with the same next parent, next part, next
batch, validation cadence, merge schedule, stopper decision, and best/frontier
state.
