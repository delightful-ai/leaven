# Layer 3 Engine/Eval Root-Cause Map

Status: canonical Layer 3 root-cause map.

Scope: engine/eval/optimizer-author substrate only. This is not the broad
project root-cause map.

Layer 3 users are optimizer authors and engine-adjacent implementors. They need
`Optimizer<P>`, `RunContext`, stage traits, graph views, eval lowering, evidence,
trust, cache, budget, errors, events, persistence, and tests to preserve the same
laws the specs promise.

## Evidence Base

- The repo contract says Leaven is spec-first, rejects compatibility shims, and
  assigns graph execution, `RunContext`, stage traits, budget, trust, cache,
  callbacks, reports, and events to `leaven-engine`: `AGENTS.md:4-8`,
  `AGENTS.md:18-19`.
- The same contract says `RunContext` is the public mutation path into
  `RunGraph`, fresh authored artifacts use `ProposalEffect::Create`, changes use
  `ProposalEffect::Change`, and tests should prove public/capability behavior:
  `AGENTS.md:38-42`.
- The review tree requires line-cited findings, no proxy proofs, no scaffolding
  acceptance, and hard cutovers: `reviews/2026-05-11-fuckery-extermination-today/AGENTS.md:14-24`.
- User alignment for this audit explicitly requires first-class eval infra,
  train/validation/test semantics, and the distinction "evals != datasets !=
  environments": `reviews/2026-05-11-fuckery-extermination-today/complaints/session-user-messages-for-codex.md:29-49`.
- User alignment also requires a power-user optimizer-author surface that does
  not make people jump through hoops or lose swappability:
  `reviews/2026-05-11-fuckery-extermination-today/complaints/session-user-messages-for-codex.md:121-125`.
- The failure that triggered the audit was a proxy path that did not actually use
  the intended Leaven surface, especially around trace/evidence/reflection:
  `reviews/2026-05-11-fuckery-extermination-today/complaints/session-user-messages-for-codex.md:582-606`.

## RC-L3-001: Public Finalization Is Split Across Two Classes Of API

- severity: blocker
- surface: `RunContext`, stage contexts, stage traits, tests

Ideal contract:

Optimizer authors implement `Optimizer<P>` and perform graph-affecting or
cost-bearing work through `RunContext`. The spec says context methods handle
graph writes, budget charges, cache lookup, callback emission, trust policy,
error normalization, event metadata, and persistence hooks:
`docs/specs/initial_library.md:1793-1916`. Stage contexts are capability-scoped,
not alternate finalization authorities: `docs/specs/initial_library.md:1918-1937`.
Optimizer law says all graph mutations happen through `RunContext` and all
costful work goes through metered stages or explicit budget charges:
`docs/specs/initial_library.md:4546-4552`.

Current implementation:

`RunContext::propose` is a real finalizer: it builds `ProposalContext`, calls the
proposer, emits stage errors, and records the metered batch through
`record_proposal_batch`: `crates/leaven-engine/src/context/run_context.rs:191-208`.
`record_proposal_batch` charges budget, records graph data, emits proposal
events, and checkpoints: `crates/leaven-engine/src/context/run_context.rs:156-188`.
`evaluate_with`/`evaluate` are also real finalizers: they check trust, resolve
sets, compute cache behavior, record requests, dispatch the evaluator, charge
cost, store evidence, record assessments, emit completion, and checkpoint:
`crates/leaven-engine/src/context/run_context.rs:344-535`,
`crates/leaven-engine/src/context/run_context.rs:609-637`.

The same public type also exposes raw context factories:
`proposal_context`, `evaluation_context`, `render_context`, and
`materialize_context`: `crates/leaven-engine/src/context/run_context.rs:286-325`.
Tests use those public factories to call dyn proposer/evaluator/materializer
paths directly: `crates/leaven-engine/tests/stage_trait_contracts.rs:17-35`,
`crates/leaven-engine/tests/stage_trait_contracts.rs:64-98`,
`crates/leaven-engine/tests/materializer_contract.rs:25-47`.

Blocker/gap:

There are legitimate-looking non-finalizing public paths beside finalizing
paths. A future optimizer author can call stage traits directly with raw
contexts and skip graph recording, evidence storage, cache, events, budget
events, and checkpointing. The API documentation hints "most callers should
prefer" finalizers, but the type system still permits the bypass:
`crates/leaven-engine/src/context/run_context.rs:286-303`.

User impact:

This recreates the exact failure pattern the audit is meant to stop: examples or
optimizer implementations can look like they use Leaven primitives while
silently bypassing the engine substrate that makes results trustworthy.

Correction direction:

Hard cut over to one public finalizing path per cost-bearing operation. Raw
context construction should be crate-private, explicit test support, or moved
behind internal dispatch modules. Public paths should be finalizers:
`RunContext::propose`, `record_proposal_batch` for already-constructed batches,
`apply_batch`, `apply_proposal`, `evaluate`, `evaluate_with`, and future
render/materialize finalizers.

Required proof/tests:

- Contract tests proving public finalizers charge budget, emit events,
  checkpoint, and mutate graph as specified.
- Compile-fail or visibility tests proving downstream crates cannot construct raw
  stage contexts except by receiving them from engine finalizers.
- Tests that object-safe dyn proposer/evaluator dispatch is still covered without
  teaching external users to call non-finalizing paths.

## RC-L3-002: Rendering/Materialization Are Split Correctly, But Not Finalized

- severity: high
- surface: render/materialize stage contract, budget, events, agentic prep

Ideal contract:

Rendering and materialization are distinct because value-returning presentation
and workspace side effects are different operations. Both may be async and
costful: `docs/specs/initial_library.md:2257-2274`. The trait laws require
costful rendering to report cost and materializers to respect read scope:
`docs/specs/initial_library.md:4585-4598`. The GEPA public/private budget
contract says every spending stage charges the central budget ledger through the
engine: `docs/specs/gepa_public_private_surface.md:840-875`.

Current implementation:

`Renderer` and `Materializer` return `Metered` values:
`crates/leaven-engine/src/stage/renderer.rs:8-26`. `RenderContext` carries a
`BudgetHandle`, but exposes only a budget snapshot and read scope:
`crates/leaven-engine/src/context/render_context.rs:8-40`. `MaterializeContext`
carries only a `BudgetSnapshot`, graph view, and read scope:
`crates/leaven-engine/src/context/materialize_context.rs:8-50`. `RunContext`
exposes raw context constructors but no public render/materialize finalizers:
`crates/leaven-engine/src/context/run_context.rs:312-325`.

Blocker/gap:

Materializer tests call `materialize_into` directly and return zero cost through
the fixture: `crates/leaven-engine/tests/materializer_contract.rs:40-47`,
`crates/leaven-engine/tests/materializer_contract.rs:190-213`. There is no
engine-level path that must charge the returned cost, emit a render/materialize
event, or checkpoint after workspace population.

User impact:

Agentic optimizers and sandboxed evaluators depend on materialization. Today
they can spend time, tokens, process calls, or workspace operations without a
central budget/event proof unless every caller remembers to account for it
manually.

Correction direction:

Add public finalizers for render/materialize work and make direct raw-context
calls internal. If stage code needs sub-rendering or sub-materialization, expose
helper finalizers on the stage context or require the outer stage's returned
`Metered` cost to include all subwork by contract.

Required proof/tests:

- `RunContext::render_with` and `RunContext::materialize_into` contract tests for
  cost charge, event emission, error normalization, and read-scope propagation.
- A nonzero-cost materializer test that fails if the central budget ledger is not
  charged.
- A hidden-partition materializer test proving forbidden evidence is not written
  to workspace output.

## RC-L3-003: Eval/Data/Environment Separation Exists In Spec But Is Partial In Code

- severity: high
- surface: `leaven-eval`, `leaven-engine`, eval lowering, future product builders

Ideal contract:

The eval lowering spec separates user input, lowered eval data, execution, and
environment: `docs/specs/eval_lowering_detail.md:24-37`. `leaven-eval` owns
dataset, splits, split-use rules, request templates, reports, and report schemas,
but not evaluator execution, workspaces, optimizer rhythm, strategy state, or
domain case semantics: `docs/specs/eval_lowering_detail.md:49-65`. Planned
modules include `plan`, `request`, `traits`, and `suite`:
`docs/specs/eval_lowering_detail.md:101-145`.

Current implementation:

`leaven-eval` correctly states that it does not execute evaluations:
`crates/leaven-eval/src/lib.rs:1-5`. It currently exports only dataset, error,
report, split, and use-policy modules: `crates/leaven-eval/src/lib.rs:7-19`.
Dataset, splits, and split-use have real starts:
`crates/leaven-eval/src/dataset.rs:29-65`,
`crates/leaven-eval/src/split.rs:59-150`,
`crates/leaven-eval/src/use_policy.rs:85-123`. The missing planned module
surface means evaluation plans, request templates, suites, and adapter traits are
not yet an implementation contract.

Blocker/gap:

The lowered eval layer is not yet complete enough to be the one place future
implementors put train/validation/test, split-use, final-test, report, and
request-template truth. If a GEPA or product-builder implementor proceeds now,
they will likely encode split semantics locally.

User impact:

The user-facing concern was precisely that evals, datasets, and environments are
not always the same thing. Partial `leaven-eval` makes that distinction easy to
state in docs but hard to preserve in code.

Correction direction:

Complete `leaven-eval` as the lowered vocabulary crate. Keep engine execution in
`leaven-engine`; keep task/workspace/environment semantics outside eval; keep
optimizer rhythm in optimizer crates. Do not invent a compatibility facade or a
parallel eval executor.

Required proof/tests:

- `leaven-eval` contract tests for dataset fingerprinting, split membership,
  split-use invariants, request template lowering, evaluation suite construction,
  and report schemas.
- Topology tests proving `leaven-eval` does not import engine/workspace/agentic
  execution crates.
- Builder/optimizer tests proving split-use intent lowers into engine
  `TrustPolicy` and engine `EvaluationRequest` values.

## RC-L3-004: Trust Is Checked On Expressions, Not Resolved Membership

- severity: blocker
- surface: trust policy, read scope, eval-set resolution, hidden splits

Ideal contract:

Agentic and optimizer stages need first-class trust boundaries so proposers do
not read or optimize against hidden test/eval data:
`docs/specs/guiding_principles.md:186-193`. Split-use policy must be reflected in
engine trust policy, and product paths must use partition requests until engine
trust can map explicit case IDs back to hidden partitions:
`docs/specs/eval_lowering_detail.md:744-757`.

Current implementation:

`TrustPolicy::check_evaluation_request` checks the unresolved
`EvaluationSet` expression for hidden partition references:
`crates/leaven-engine/src/trust.rs:119-145`. Its recursive collector treats
`EvaluationSet::Cases`, `Tagged`, `Recent`, and `Unscoped` as non-partition
references: `crates/leaven-engine/src/trust.rs:154-182`. `CaseSet::resolve`
accepts explicit case IDs by checking only existence:
`crates/leaven-engine/src/case_set.rs:64-70`. Graph views hide assessments based
on the stored unresolved expression, not resolved case membership:
`crates/leaven-engine/src/graph/view.rs:254-295`.

Blocker/gap:

An optimizer can request hidden cases by explicit ID. The test suite currently
documents that candidate-scoped sets do not expose hidden partitions:
`crates/leaven-engine/tests/trust_policy.rs:113-130`. That is only true at the
expression level and is unsafe for split-sensitive runs.

User impact:

Layer 3 users build `EvaluationRequest`s manually. A manual request over
`EvaluationSet::Cases(test_ids)` can pass trust checks even when test is supposed
to be final-report-only. That can contaminate search, parent selection, part
selection, acceptance, and population state.

Correction direction:

Enforce trust after resolution. `CaseSet` or `ResolvedEvaluationSet` must carry
partition membership metadata, and `TrustPolicy` must reject resolved sets
containing hidden members for the requesting actor/use. Split-use policy should
lower into engine-readable trust/read/evidence-use rules.

Required proof/tests:

- A regression where a hidden `TEST` partition is requested through
  `EvaluationSet::Cases(test_id)` and is rejected.
- Read-scope tests proving assessment/evaluation-request visibility is based on
  resolved membership, including `All`, `Recent`, `Sample`, `Union`,
  `Intersect`, and `Difference`.
- Split-use tests for `ProposerFeedback`, `ParentSelection`, `PartSelection`,
  `CandidateAcceptance`, `PopulationObservation`, `Report`, `EvaluatorOnly`, and
  `FinalTest`.

## RC-L3-005: Cache Identity Is Not Request/Graph Semantics

- severity: blocker
- surface: evaluation cache, reports, graph truth

Ideal contract:

The cache key must include evaluator fingerprint, resolved evaluation set,
request shape, and artifact cache identities:
`docs/specs/gepa_optimizer_surface.md:535-541`. Core evaluation docs say
independent, pairwise, and listwise requests are distinct and must not be
silently coerced: `crates/leaven-core/src/evaluation.rs:170-184`. Pair order is
semantically meaningful when ordered: `crates/leaven-core/src/evaluation.rs:319-332`.
Reports should point at graph truth, not duplicate or invent it:
`docs/specs/initial_library.md:1984-1996`.

Current implementation:

`EvaluationCacheKey` includes evaluator fingerprint, policy, case-set version,
case IDs, and candidate cache identities: `crates/leaven-engine/src/cache.rs:46-59`.
`evaluation_cache_key` builds keys from those fields only:
`crates/leaven-engine/src/context/run_context.rs:781-794`. It reduces
independent/listwise requests to a candidate list and pairwise requests to
`[left, right]`: `crates/leaven-engine/src/context/run_context.rs:797-824`.
Cache hits return cached assessment IDs with the new request ID and zero cost,
without recording new assessments or graph-visible reuse lineage:
`crates/leaven-engine/src/context/run_context.rs:537-568`.

Blocker/gap:

The key omits request kind, granularity, purpose, pair/list semantics, and
assessment shape. Even when the key is accidentally distinct by candidate order,
the cache-hit report can associate request N with assessment IDs recorded for
request M.

User impact:

An optimizer can update population or acceptance logic from assessment IDs that
do not belong to the request it just made. That is silent graph corruption at the
report layer: the event says evaluation completed for request N, but the graph
assessment records point elsewhere.

Correction direction:

Expand `EvaluationCacheKey` to include full resolved request semantics. On hit,
make reuse graph-visible: either record derived/alias assessment records for the
new request or return an explicit "request N reused request M assessments" report
and event that downstream code cannot mistake for fresh assessment records.

Required proof/tests:

- Independent vs pairwise vs listwise over the same candidates/cases cannot share
  cache entries.
- Aggregate vs per-case vs both cannot share entries unless a named evaluator law
  proves safe projection.
- Ordered pair `(A, B)` and `(B, A)` remain distinct; unordered symmetry is
  explicit.
- Cache hits preserve graph/request truth and are visible in reports/events.

## RC-L3-006: Evidence Is Recorded, But Reflection Cannot Consume It Honestly

- severity: blocker
- surface: evidence store, proposer context, reflection/proposal contract

Ideal contract:

Evidence is not assumed to be a number, and preference is a separate concept
built on top of evidence: `docs/specs/guiding_principles.md:114-125`. Proposers
should receive owned requests and construct rich views from `ctx.graph()`:
`docs/specs/initial_library.md:2172-2198`. Proposal provenance distinguishes
causal lineage from `informed_by` references so a proposer can record what it
read: `crates/leaven-core/src/proposal.rs:282-387`.

Current implementation:

`RunContext::record_assessments` stores evidence through `EvidenceStore` and
records evidence refs in graph assessments:
`crates/leaven-engine/src/context/run_context.rs:609-637`.
`RunGraphView` exposes read-scoped assessment records and evidence refs:
`crates/leaven-engine/src/graph/view.rs:204-232`,
`crates/leaven-engine/src/graph/view.rs:407-425`. The only typed evidence
payload accessor is `RunContext::assessment_evidence`:
`crates/leaven-engine/src/context/run_context.rs:640-652`.
`ProposalContext` exposes graph, read scope, budget, render context, and
materialize context, but no evidence reader:
`crates/leaven-engine/src/context/proposal_context.rs:8-62`.

Blocker/gap:

A proposer can see evidence refs but cannot load evidence payloads from the
context it actually receives. GEPA currently avoids the problem by using a local
`SurfaceProposer` that only sees artifact/surface/part and a fixed edit fixture:
`crates/leaven-gepa/src/proposer.rs:6-19`,
`crates/leaven-gepa/src/proposer.rs:21-56`. That path cannot express reflective
mutation over scored feedback, traces, or selected evidence.

User impact:

Real optimizers need evidence to drive reflection, parent/part choice, and
provenance. Without a single evidence-to-reflection contract, every optimizer
will invent a local request shape and bypass the engine surface.

Correction direction:

Choose and encode the contract. The stronger Layer 3 contract is: optimizers
select visible assessments/evidence; stage-owned renderers/materializers lower
them into a request/view; `ProposalContext` can load or render scoped evidence
without mutation authority; `RunContext` finalizes the resulting proposals and
records `InfoRef::Assessment` references. If evidence preloading remains the
chosen path, make it the only path and remove misleading evidence refs from
proposer-facing views.

Required proof/tests:

- A proposer can produce a proposal informed by a visible assessment and the
  graph records `InfoRef::Assessment`.
- The same proposer cannot read hidden-partition evidence through context,
  rendering, or materialization.
- GEPA reflective mutation can consume selected feedback/trace evidence without
  a GEPA-local graph/evidence escape hatch.

## RC-L3-007: Tests Prove Many Pieces, But Not The Layer 3 Contract As A Whole

- severity: high
- surface: contract tests and proof gates

Ideal contract:

Tests should assert public/capability behavior unless an invariant is genuinely
private: `AGENTS.md:41-42`. The test design skill says every test must kill a
family of wrong implementations and assert each fact at the lowest level where it
can be expressed cleanly. The design validation procedure requires implementations
to compile, run, and match optimizer descriptions without modifying the library:
`docs/specs/guiding_principles.md:353-374`.

Current implementation:

There are good focused tests for proposal finalization:
`crates/leaven-engine/tests/context_services.rs:28-49`, evaluation recording and
evidence storage: `crates/leaven-engine/tests/context_services.rs:124-208`,
pairwise/listwise graph targets: `crates/leaven-engine/tests/context_services.rs:210-309`,
eval cache reuse: `crates/leaven-engine/tests/context_services.rs:454-492`,
engine evaluator dispatch: `crates/leaven-engine/tests/evaluator_registry.rs:24-66`,
and lowered split policy: `crates/leaven-eval/tests/split_contract.rs:178-224`.

The same suite also legitimizes raw context use:
`crates/leaven-engine/tests/stage_trait_contracts.rs:17-35`,
`crates/leaven-engine/tests/stage_trait_contracts.rs:64-98`,
`crates/leaven-engine/tests/materializer_contract.rs:25-47`. The cache tests
expect reused assessment IDs across requests and do not assert graph-visible
reuse lineage: `crates/leaven-engine/tests/context_services.rs:454-492`,
`crates/leaven-engine/tests/evaluator_registry.rs:138-163`.

Blocker/gap:

The suite proves useful substrate pieces, but it does not prove the complete
Layer 3 public contract: no raw bypasses, post-resolution trust, cache-hit graph
truth, render/materialize budget finalization, evidence-to-reflection, and
non-GEPA optimizer expressibility.

User impact:

Future implementors can satisfy nearby tests while preserving the same root
failure: a proxy path works, but the intended optimizer-author surface still
does not.

Correction direction:

Replace proxy proof with contract proof. Raw dispatch tests should move inward;
public tests should exercise finalizers and the graph/evidence/cache/trust/budget
invariants users depend on.

Required proof/tests:

- One scalar keep-best optimizer over public Layer 3 primitives.
- One pairwise tournament optimizer using `EvaluationRequest::Pairwise`, graph
  assessment IDs, and a population/preference path.
- One reflective proposer over selected evidence/traces with hidden split
  enforcement.
- One materializing agentic-style stage with nonzero budget and no hidden
  evidence leak.
