# Open Design Questions

Status: integrated refinement pass.

These are questions the audit should not hide inside implementation. If an
implementor guesses wrong, the public surface can drift again.

## Q1: What Is The Layer 1 Runtime Composition Type?

The LM spec currently shows direct `OpenAiLm` plus `CachedLm`, while the audit
flags `CachedLm` as a bad ordinary-user smell.

Decision needed:

- Do ordinary users configure cache/runtime through the `optimize(...)` builder?
- Is there a public `LmRuntime` / `RuntimeRole` composition type?
- Are solver LM and reflector LM independent roles with independent cache and
  budget policies?

Likely direction: keep `CachedLm<M, C>` as an advanced implementation wrapper,
but make ordinary examples configure solver/reflector runtime/cache by role.

## Q2: Does Public `.score(...)` Return `Score` Or `Assessment`?

User vocabulary wants score. Core vocabulary wants assessment/evidence.

Decision needed:

- Is `Score` a Layer 1 facade only?
- What is the exact `IntoScore` / `IntoAssessmentEvidence` lowering contract?
- How does natural-language feedback avoid becoming hidden scalar metadata?

Likely direction: keep `.score(...)` and `Score` for ordinary users, but make
the type rich and explicitly lower into assessment/evidence records.

## Q3: How Does A Reflector Receive Evidence?

Decision needed:

- Which layer owns evidence selection?
- Which layer owns rendering into LM messages or agent workspaces?
- Does `ProposalContext` load scoped evidence payloads, or does GEPA pass a
  complete owned evidence view?
- How are selected evidence refs preserved for `informed_by` without giving the
  proposer graph mutation authority?
- How does the same contract work for LM reflection and agentic reflection?

Likely direction: GEPA owns evidence/trace selection, renderers own
presentation, the proposer receives owned request data plus limited
`ProposalContext` capabilities for budget/render helpers, and `RunContext`
finalizes graph mutation.

This should become an explicit request/response contract before implementation,
not a hidden convention inside GEPA.

## Q4: What Is The Minimum Product Proof Before Live AIME?

Live AIME is useful, but it is expensive and can hide proxy paths.

Decision needed:

- What mocked LM proof must pass before live OpenAI/Qwen/etc. is meaningful?
- Does AIME prove Layer 1 only, or also GEPA reflection and LM runtime/cache?
- What makes "numbers go up" acceptable evidence rather than fixture behavior?

Likely direction: require a deterministic mock-LM reflective run that consumes
casewise feedback and emits a new edit before live AIME is treated as product
proof.

## Q5: Which Placeholder Names May Stay Public?

Some scaffolding is useful. The problem is public false affordance.

Decision needed:

- Is there any approved public scaffolding namespace?
- Should placeholder crates be removed from workspace features until real?
- Should topology tests deny empty public structs by default?

Likely direction: public placeholders stay only behind explicit `scaffold` or
test/demo modules; ordinary facades and defaults expose only real contracts.

## Q6: What Is The Exact Single-Task Mode?

The public/private spec says train cases or one unscoped task are both Layer 1.
The current builder centers `.train(...)`.

Decision needed:

- Is single-task `.task(...)`, `.case(...)`, `.environment(...)`, or a mode on
  `Gepa`?
- Does single-task create a no-dataset eval plan, a singleton dataset, or an
  evaluator-owned environment?
- How does final reporting look when there is no dataset?

Likely direction: single-task is no-dataset unless concrete cases are supplied;
do not fake a one-row training set as the public story.

## Q7: What Are Cache-Hit Graph Semantics?

Engine evaluation cache and LM response cache are separate, but both can hide
work if their hits are invisible.

Decision needed:

- When evaluation cache returns an existing assessment, does the current request
  record a graph event or edge to that reused assessment?
- Does the cache key include request kind, granularity, purpose, pair/list
  order, assessment shape, evaluator fingerprint, candidate identity, and case
  identity?
- How are LM response cache hits charged or reported?
- How does a result facade report cache hits without implying fresh evaluation?

Likely direction: cache hits are observable run events with graph-visible
lineage to reused identities. They are never invisible early returns.

## Q8: Where Does `leaven-lm-cache` Compose With GEPA?

The specs currently create tension: GEPA should consume provider-neutral LM
capabilities, while cache policy is a reusable LM wrapper/backend and should not
become GEPA rhythm.

Decision needed:

- Should `leaven-gepa` depend on `leaven-lm-cache`?
- Does cache composition happen in `leaven-run`, an LM runtime builder, or user
  code?
- How do solver, reflector, and scorer/model-judge cache policies differ?

Likely direction: GEPA consumes `leaven-lm` or agent/runtime capabilities.
Cache composition lives above GEPA, through runtime roles or advanced wrappers.

## Q9: What Is The Public Scaffolding Policy?

Decision needed:

- Is public scaffolding ever allowed outside explicitly named scaffold/test
  namespaces?
- Should default `leaven` features be forbidden from exposing scaffold names?
- What test scans prevent empty public structs and skeleton docs from becoming
  product surface?

Likely direction: use the categories in `public-maturity-gates.md`; default
facades expose only ordinary public contracts.
