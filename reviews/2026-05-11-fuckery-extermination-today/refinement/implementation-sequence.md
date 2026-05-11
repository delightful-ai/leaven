# Implementation Sequence Implied By The Refined Audit

Status: integrated refinement pass.

This sequence is ordered to restore the original vision with the fewest false
positive proof paths. It is not a compatibility plan; Leaven uses hard cutovers.

## Phase 0: Remove Or Quarantine False Public Proof

Goal: stop examples and public names from proving the wrong thing.

Required changes:

- rename or move fixed GEPA mutation fixtures out of production-looking
  `ReflectiveMutation`;
- mark deterministic proposer examples as fixture demos until real reflection
  exists;
- remove the Python OpenAI bypass from the AIME proof path or mark it as an
  external helper that is not Leaven product proof;
- remove derive macros from default-facing public imports until implemented;
- remove placeholder provider/backend/standard exports from ordinary facades.
- distinguish true placeholders from stale skeleton metadata on behavior-bearing
  crates before deleting anything.

Exit criteria:

- no ordinary example claims GEPA reflection unless an LM/agent reflector reads
  selected evidence/trace context through Leaven surfaces;
- no default import exposes compile-error derives or inert standard names as
  normal capability;
- default `leaven` imports expose no placeholder provider/backend types.

## Phase 1: Restore Layer 1 Product Builder Truth

Goal: make `optimize(seed)` a real ordinary-user surface.

Required changes:

- async runner contract;
- async score/evaluator helper contract;
- rich `Score` facade lowering into assessment/evidence/preference;
- typed `ScoreContext` accessors for trace/history/output/error/case/budget;
- stable train/validation/test case identity;
- single-task/no-dataset mode;
- runtime/cache policy wiring for solver and reflector roles;
- ordinary prelude separated from engine-author prelude;
- reports distinguish absent/failed evidence from numeric zero.

Exit criteria:

- a mocked LM AIME-like run can use the ordinary builder, Leaven LM runtime,
  response cache, GEPA optimizer value, and result report with no shell escape;
- swapping mock LM to OpenAI is a provider construction change, not an example
  architecture change.

## Phase 2: Seal Engine And Eval Invariants

Goal: make optimizer authors safe by default, and make GEPA possible to trust.

Required changes:

- raw proposal/evaluation/render/materialize contexts become private/test-only
  or explicitly non-finalizing;
- `RunContext` has public finalizers for every metered stage path;
- proposer context either gets scoped evidence loading or proposer requests are
  required to contain complete scoped evidence views;
- explicit case IDs cannot bypass hidden validation/test partitions;
- split-use/evidence-use policy is enforced after resolution, not only on the
  syntax of `EvaluationSet`;
- evaluation cache key includes request semantics, granularity, purpose, pair
  order/symmetry, and assessment shape;
- cache hits create graph-visible linkage from the current request to reused
  assessment/evidence identity;
- `leaven-eval` owns stable dataset/split/plan/report lowering and completes
  the missing module/lowering surface;
- public tests assert `RunContext` invariant behavior, not just raw stage
  object-safety;
- evidence/preference/population contract tests prove non-scalar paths.

Exit criteria:

- a non-GEPA optimizer can implement pairwise tournament over
  `EvaluationRequest::Pairwise` without new engine/core traits;
- hidden split/trust laws are proven at the engine boundary;
- a cached evaluation cannot silently reuse an assessment for a semantically
  different request;
- renderer/materializer costs are charged through public finalizers.

## Phase 3: Restore GEPA As One Honest Optimizer

Goal: make GEPA a reusable optimizer value with real swappable slots.

Required changes:

- GEPA builder supports surface, parent selector, part selector, batch sampler,
  reflector/proposer, acceptance, validation policy, population/frontier, merge,
  stopper/config;
- real reflective mutation request with parent, part view, assessment IDs,
  selected evidence/trace views, attribution, background, and budget/runtime
  handles;
- GEPA proposal generation routes through `RunContext` finalization or an
  equivalent invariant-preserving helper;
- acceptance consumes assessment/evidence/preference context, not two `f64`s;
- population observation is evidence-shape-neutral and not scalar-only;
- validation/test hiding is enforced by split/trust policy.

Exit criteria:

- deterministic proposer milestone still exists only as an explicitly named
  fixture or milestone test;
- mock-LM reflective GEPA can improve a simple artifact using actual feedback;
- `p8`/AIME proof uses the same surfaces as a real user.

## Phase 4: Topology And Placeholder Hardening

Goal: make the crate graph enforce truth instead of only naming future work.

Required changes:

- delete or hard-cut `crates/leaven-dsrs` into a real workspace crate;
- extend topology tests to reject orphan `crates/*` dirs;
- add a public-stub ledger/deny test for unapproved empty public structs,
  skeleton module docs, compile-error macros, and provider/backend feature
  exports;
- make crate roots private-by-default module maps with curated re-exports;
- reconcile GEPA/cache dependency direction across spec, manifests, and tests.

Exit criteria:

- the topology gate fails on public capability names that are only placeholders;
- docs/specs and live manifests agree about every crate boundary.

## Phase 5: Re-run The Original Pressure Tests

Goal: prove the original vision, not just GEPA.

Required proof set:

- scalar keep-best single-task;
- pairwise tournament with fitted population/preference model;
- GEPA parity with real reflective mutation;
- agentic Git/skill artifact with materialization, evidence store, trust
  boundaries, and final-test policy.

Evidence: `docs/specs/initial_library.md:4634-4685`.

Exit criteria:

- a competent implementor can implement a new optimizer paper using public
  primitives without changing core/engine;
- examples show real optimizer behavior, not proxy score movement.
