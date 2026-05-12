# Layer 3 Engine/Eval Fix-Priority Map

Status: canonical ordered fix map for Layer 3 engine/eval/optimizer-author work.

This map orders fixes by dependency and proof value. It intentionally does not
plan compatibility shims. Each item is a hard cutover target and names the proof
gate required before downstream GEPA or Layer 1 claims can trust it.

## Ordering Rule

Do not restore GEPA as the trusted product proof before the engine/eval substrate
is sealed. The original implementation plan put pairwise tournament before GEPA
parity because pairwise stresses the new design that Python GEPA does not cover:
`docs/specs/initial_library.md:4638-4683`. The GEPA optimizer spec says today's
one-step GEPA-like flow proves substrate shape but is not the library product:
`docs/specs/gepa_optimizer_surface.md:38-52`.

## P0: Freeze The Layer 3 Public Authority Boundary

- priority: blocker
- owns: `RunContext`, raw stage contexts, public tests

Ideal contract:

`RunContext` is the public mutation/finalization authority. Context methods
handle graph writes, budget, cache, callbacks, trust, errors, events, and
persistence: `docs/specs/initial_library.md:1827-1916`.

Current implementation:

The authority exists but is split. `RunContext::propose` finalizes proposer
output through `record_proposal_batch`:
`crates/leaven-engine/src/context/run_context.rs:191-208`.
`RunContext::evaluate`/`evaluate_with` finalize evaluator output:
`crates/leaven-engine/src/context/run_context.rs:344-535`. Raw stage contexts are
also public: `crates/leaven-engine/src/context/run_context.rs:286-325`.

Blocker/gap:

Public raw context factories are a bypass around the exact finalization rules
Layer 3 users need.

Correction direction:

- Make raw context factory construction private or explicit test/internal support.
- Keep or add only finalizing public APIs:
  - proposal finalization;
  - already-built proposal batch finalization;
  - apply finalization;
  - evaluation finalization;
  - render/materialize finalization;
  - explicit budget charge;
  - checkpoint with optimizer state.
- Rename any remaining raw path to include "non_finalizing" if it must be
  reachable internally.

Required proof/tests:

- Downstream/public API tests cannot call raw context factories.
- Public finalizer tests prove budget charge, event emission, checkpointing,
  graph mutation, and typed error behavior.
- Dyn proposer/evaluator object-safety tests use an internal helper, not the
  public optimizer-author path.

Exit gate:

No public or example code can run proposer/evaluator/render/materialize work
without either a `RunContext` finalizer or an explicit stage-owned metered charge
contract.

## P1: Add Render And Materialize Finalizers

- priority: blocker
- owns: render/materialize work, workspace prep, budget events

Ideal contract:

Rendering and materialization are async, costful, and intentionally distinct:
`docs/specs/initial_library.md:2257-2274`. Every stage that spends cost must
charge the central ledger through the engine:
`docs/specs/gepa_public_private_surface.md:860-875`.

Current implementation:

`Renderer` and `Materializer` return `Metered` values:
`crates/leaven-engine/src/stage/renderer.rs:8-26`. `RunContext` exposes raw
render/materialize contexts and no finalizer:
`crates/leaven-engine/src/context/run_context.rs:312-325`. `MaterializeContext`
has only a `BudgetSnapshot`: `crates/leaven-engine/src/context/materialize_context.rs:8-50`.

Blocker/gap:

Costful workspace/render preparation can be invisible to budget, events, and
checkpointing.

Correction direction:

- Add `RunContext` finalizers for value rendering and workspace materialization.
- Add events or event metadata for rendered/materialized stage work.
- Decide whether stage-context sub-rendering/sub-materialization charges directly
  or must be folded into the outer stage's returned `Metered` cost. Encode one
  law, not both as an implicit convention.

Required proof/tests:

- A nonzero-cost renderer/materializer charges budget exactly once.
- Failures emit typed stage errors and do not produce misleading success events.
- Hidden evidence is not rendered/materialized for forbidden read scopes.

Exit gate:

An agentic or sandboxed stage can prepare workspace files while the graph,
budget, event stream, and read scope all agree on what happened.

## P2: Fix Evaluation Cache Semantics And Graph Truth

- priority: blocker
- owns: `EvaluationCacheKey`, cache-hit reports, graph/event semantics

Ideal contract:

Cache keys use evaluator fingerprint, request fingerprint/resolved-set identity,
request shape, and artifact cache identities:
`docs/specs/initial_library.md:3124-3135`,
`docs/specs/gepa_optimizer_surface.md:535-541`. Requests of different shape are
not equivalent: `crates/leaven-core/src/evaluation.rs:170-184`. Reports point at
graph truth: `docs/specs/initial_library.md:1984-1996`.

Current implementation:

`EvaluationCacheKey` omits request fingerprint/resolved-set identity, request
kind, granularity, purpose, explicit pair-order/symmetry policy, listwise
grouping semantics, and assessment shape:
`crates/leaven-engine/src/cache.rs:46-59`. `evaluation_cache_key` constructs only
evaluator, policy, case-set version, case IDs, and request-ordered candidate
cache identities:
`crates/leaven-engine/src/context/run_context.rs:781-824`. On hit, the report
returns cached assessment IDs for a new request without graph-visible reuse
linkage: `crates/leaven-engine/src/context/run_context.rs:537-568`.

Blocker/gap:

Semantically different evaluations can collide, and even safe hits can make
request N appear to own assessment IDs recorded for request M.

Correction direction:

- Expand cache key to include request fingerprint or resolved-set identity,
  resolved request kind, granularity, purpose, pair order or unordered symmetry,
  listwise grouping semantics, assessment shape, evaluator fingerprint,
  candidate identities, case identities, and case-set version.
- Add graph-visible cache-hit semantics:
  - either alias/derived assessment records for the new request;
  - or explicit reused-assessment linkage in report and event types.
- Keep default `CachePolicy::Never` for stochastic evaluators unless the
  evaluator declares deterministic/fingerprinted behavior.

Required proof/tests:

- Independent/listwise/pairwise requests over the same ids do not collide.
- Aggregate/per-case/both do not collide without an explicit projection law.
- Ordered pair reversals remain distinct by named request semantics, not only by
  incidental candidate vector order; unordered symmetry is explicit.
- Cache hit reports cannot be mistaken for fresh request-owned assessment
  records.

Exit gate:

Every cache hit is semantically keyed and graph-visible.

## P3: Enforce Trust After Evaluation-Set Resolution

- priority: blocker
- owns: `TrustPolicy`, `CaseSet`, `ResolvedEvaluationSet`, `ReadScope`

Ideal contract:

Hidden validation/test data must remain hidden from optimizer/proposer search,
and split-use policy must lower into engine trust/read enforcement:
`docs/specs/eval_lowering_detail.md:344-397`,
`docs/specs/eval_lowering_detail.md:744-757`.

Current implementation:

Trust checks unresolved expressions:
`crates/leaven-engine/src/trust.rs:119-145`. Explicit case IDs are treated as
non-partition references: `crates/leaven-engine/src/trust.rs:154-182`. `CaseSet`
resolves explicit case IDs by existence only:
`crates/leaven-engine/src/case_set.rs:64-70`. Graph visibility also checks the
stored expression instead of resolved membership:
`crates/leaven-engine/src/graph/view.rs:254-295`.

Blocker/gap:

Hidden cases can be requested as explicit IDs, and dynamic sets cannot be
checked against split-use policy after resolution.

Correction direction:

- Add resolved partition-membership metadata to `CaseSet` resolution or
  `ResolvedEvaluationSet`.
- Move hidden-partition enforcement to resolved sets.
- Lower `SplitUsePolicy` into engine-readable rules for request purpose and
  evidence/read visibility.
- Replace tests that bless explicit case IDs as safe with tests that prove
  hidden membership refusal.

Required proof/tests:

- `EvaluationSet::Cases([test_id])` is rejected for optimizer/proposer search
  when `TEST` is hidden/final-report-only.
- `All`, `Recent`, `Sample`, `Union`, `Intersect`, and `Difference` enforce
  hidden membership after resolution.
- `FinalTest` can run only in the allowed final-report trust mode.

Exit gate:

Trust is about actual resolved data exposure, not the syntactic shape of the
request.

## P4: Complete The Lowered Eval Crate Boundary

- priority: high
- owns: `crates/leaven-eval`, eval/data/report vocabulary

Ideal contract:

`leaven-eval` owns dataset/splits/split-use/request-template/report vocabulary
and not execution, workspaces, optimizer rhythm, or domain semantics:
`docs/specs/eval_lowering_detail.md:49-65`. Planned modules include `plan`,
`request`, `traits`, and `suite`: `docs/specs/eval_lowering_detail.md:101-145`.

Current implementation:

The crate currently exports dataset, error, report, split, and use-policy only:
`crates/leaven-eval/src/lib.rs:7-19`. Dataset/split/use-policy pieces are real
but incomplete relative to the spec:
`crates/leaven-eval/src/dataset.rs:29-115`,
`crates/leaven-eval/src/split.rs:59-150`,
`crates/leaven-eval/src/use_policy.rs:85-123`.
`leaven-run` already fills the missing lowering locally by constructing
`Dataset`, `DatasetSplits`, engine `CaseSet`, trust policy, final evaluation
requests, and split reports in the builder:
`crates/leaven-run/src/builder.rs:214-240`,
`crates/leaven-run/src/builder.rs:321-355`,
`crates/leaven-run/src/builder.rs:580-605`.

Blocker/gap:

There is no implemented canonical home for evaluation plans, request templates,
suites, and the complete report contract. The current product builder is already
becoming that hidden home. That invites local lowerings in GEPA or future product
builders and makes report semantics depend on unresolved request syntax instead
of resolved split/eval truth.

Correction direction:

- Add `plan.rs`, `request.rs`, `suite.rs`, and `traits.rs`.
- Keep execution out of `leaven-eval`.
- Make reports cite graph IDs/evidence refs and split-use summaries, not
  duplicated evidence payload truth.
- Add fingerprints for dataset/split/plan metadata that change when product
  semantics change.

Required proof/tests:

- Dataset/split/request-template/suite law tests.
- Topology contract: `leaven-eval` does not depend on engine, run, GEPA,
  workspace, agentic, LM, or provider crates.
- Product lowering smoke: eval suite can lower to engine `CaseSet`,
  `EvaluationRequest`, and trust policy without reverse dependencies.
- Report lowering smoke: split reports are derived from resolved eval/split
  membership, not only from `EvaluationSet::Partition` in the original request.

Exit gate:

Future implementors do not need to guess where eval/data/report facts belong.

## P5: Define The Evidence-To-Reflection Contract

- priority: high
- owns: `ProposalContext`, evidence store access, render/materialize, GEPA reflection

Ideal contract:

Evidence shape is neutral, preference is separate, and proposal/reflection is a
swappable strategy choice: `docs/specs/guiding_principles.md:114-139`.
Proposers get typed request shapes and build rich views from graph context:
`docs/specs/initial_library.md:2172-2198`. Proposal provenance records both
causal lineage and `informed_by`: `crates/leaven-core/src/proposal.rs:282-387`.

Current implementation:

Evidence is stored and graph refs are recorded:
`crates/leaven-engine/src/context/run_context.rs:609-637`.
`ProposalContext` has no evidence reader:
`crates/leaven-engine/src/context/proposal_context.rs:8-62`.
GEPA reflection uses a local `SurfaceProposer` over artifact/surface/part and a
fixed-edit `ReflectiveMutation` fixture:
`crates/leaven-gepa/src/proposer.rs:6-56`.

Blocker/gap:

There is no canonical way for a reflector/proposer to consume selected
assessment evidence, traces, or feedback while preserving read scope and
`InfoRef` provenance.

Correction direction:

- Make one owner explicit:
  - optimizer selects assessments/evidence and builds a complete owned request;
  - or `ProposalContext` exposes scoped evidence loading/rendering.
- Preferred Layer 3 contract: proposer context may load/render visible evidence
  payloads by `AssessmentId`, but cannot mutate graph. `RunContext` remains the
  finalizer and records `InfoRef::Assessment`.
- GEPA reflection must use this contract instead of a GEPA-local narrow
  `SurfaceProposer` path.

Required proof/tests:

- Reflective proposer reads one visible assessment and emits a proposal with
  `InfoRef::Assessment`.
- Hidden evidence cannot be loaded or materialized.
- GEPA reflection can be backed by an async LM or agent role without changing
  engine/core traits.

Exit gate:

Trace/evidence-driven reflection is no longer a local GEPA workaround.

## P6: Seal Budget And Error Contracts

- priority: high
- owns: budget ledger, stage handles, typed errors

Ideal contract:

Budget tracks all stages and is independent from stopping:
`docs/specs/guiding_principles.md:195-199`. Every spending stage reports costs:
`docs/specs/guiding_principles.md:345-347`. Known capability failures use typed
errors at the boundary: `docs/specs/gepa_public_private_surface.md:570-578`.

Current implementation:

`BudgetLedger::charge` enforces metric, LM, and seconds caps and records per
stage totals: `crates/leaven-engine/src/budget.rs:42-80`. `BudgetHandle` can
charge or create substages: `crates/leaven-engine/src/budget.rs:90-115`.
`RunContext::charge` emits budget events or stopped-run errors:
`crates/leaven-engine/src/context/run_context.rs:265-284`. Current proposer and
evaluator errors still have broad `Message` variants:
`crates/leaven-engine/src/stage/proposer.rs:92-125`,
`crates/leaven-engine/src/stage/evaluator.rs:77-104`.

Blocker/gap:

Budget is real for proposal/evaluation finalizers, but render/materialize and
nested stage costs are not sealed. Error variants are not yet specific enough
for unsupported granularity, request-shape mismatch, trust refusal after
resolution, cache-key refusal, attachment/materialization failure, and
evidence-projection refusal.

Correction direction:

- Route all costful finalizers through budget.
- Add typed refusal variants where optimizer authors need decision-making
  signals.
- Keep generic message variants only for genuinely unclassified edges.

Required proof/tests:

- Budget exhaustion before proposal graph mutation.
- Budget exhaustion after evaluation request recording but before assessment
  mutation, if evaluator cost returns over budget.
- Typed errors for unsupported request shape/granularity and materialization
  refusal.

Exit gate:

Layer 3 code can decide retry/skip/abort/report from typed errors and budget
state, not debug strings.

## P7: Prove Evidence, Preference, And Population Through Graph IDs

- priority: high
- owns: standard evidence, preference relation, population observation

Ideal contract:

Evidence is measurement, preference interprets evidence, and population/frontier
state is optimizer-owned strategy state:
`docs/specs/guiding_principles.md:114-125`,
`docs/specs/initial_library.md:70-71`. The score-normalization contract preserves
score axes, feedback, attachments, metadata, and diagnostics until explicit
projection: `docs/specs/eval_lowering_detail.md:315-343`.

Current implementation:

The engine has graph-backed `PreferenceRelation<P>` and `Population<P>` traits:
`crates/leaven-engine/src/stage/preference.rs:8-17`,
`crates/leaven-engine/src/stage/population.rs:8-38`. `leaven-evidence` has real
casewise and pairwise starts:
`crates/leaven-evidence/src/casewise.rs:36-78`,
`crates/leaven-evidence/src/pairwise.rs:17-91`, but also root-exports empty
placeholder vocabulary: `crates/leaven-evidence/src/lib.rs:36-77`. The current
high-level run path fixes evidence to scalar casewise feedback:
`crates/leaven-run/src/builder.rs:43-51`,
`crates/leaven-run/src/evidence.rs:23-32`,
`crates/leaven-run/src/evaluator.rs:65-128`.

Blocker/gap:

There is no minimum graph-backed proof that scalar and pairwise evidence drive
preference/population decisions through assessment IDs. Concrete helper crates
can still update local population structs directly, which is useful scaffolding
but not the Layer 3 contract.

Correction direction:

- Keep public `.score(...)` as a Layer 1 facade, not internal optimizer truth.
- Implement or remove production-looking placeholder evidence exports.
- Add graph-backed scalar and pairwise preference/population contract tests.
- Make population implementations that claim engine compatibility observe
  `AssessmentId` plus `RunGraphView`, not just direct evidence payloads.

Required proof/tests:

- Scalar casewise evidence updates a graph-backed keep-best/preference path from
  assessment IDs.
- Pairwise judgment evidence updates a graph-backed tournament/preference path
  from assessment IDs.
- Rich score facade lowering preserves comparable axes, feedback, attachments,
  metadata, and evidence refs before report projection.

Exit gate:

GEPA and non-GEPA optimizers can rely on the same standard evidence/preference
and population substrate instead of binding to scalar helper shortcuts.

## P8: Replace Proxy Proofs With Optimizer-Author Contract Proofs

- priority: high
- owns: test suite, examples, future GEPA trust gate

Ideal contract:

The design is right only if optimizers from the target set compile and run over
the public primitives without changing the library:
`docs/specs/guiding_principles.md:353-374`. P1 proves
`Optimizer + RunContext + RunGraph`, P2 proves pairwise/tournament, P3 proves
GEPA parity on top of that substrate: `docs/specs/initial_library.md:4638-4683`.

Current implementation:

The suite has useful focused tests, but cache tests currently allow one
assessment record to serve two requests:
`crates/leaven-engine/tests/evaluator_registry.rs:138-163`, and raw context tests
exercise non-finalizing paths:
`crates/leaven-engine/tests/stage_trait_contracts.rs:17-98`.

Blocker/gap:

Focused tests can pass while the public optimizer-author contract still fails.

Correction direction:

- Add contract examples/tests for:
  - scalar keep-best;
  - pairwise tournament;
  - reflective evidence-driven proposer;
  - materializing agentic-style evaluator/proposer;
  - final GEPA after P0-P7.
- Tests should fail if they bypass finalizers, hidden trust, or graph-visible
  cache semantics.

Required proof/tests:

- A non-GEPA optimizer using `EvaluationRequest::Pairwise` compiles and runs with
  no new engine/core traits.
- A reflective proposer consumes actual graph evidence and records provenance.
- GEPA restoration is accepted only after the engine/eval gates above pass.

Exit gate:

The proof suite demonstrates the original Layer 3 vision, not a nearby example
that happens to move a metric.
