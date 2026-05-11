# Layer 3 Evidence, Trust, Budget, And Cache

Status: active findings recorded.

This file audits whether internal engine support surfaces preserve evidence,
trust/read scopes, budget accounting, and cache behavior without leaking
implementation detail into ordinary user APIs.

## Findings

### L3-004: `ProposalContext` cannot load scoped evidence payloads

- severity: high
- evidence: `crates/leaven-engine/src/context/proposal_context.rs:8-62`,
  `crates/leaven-engine/src/graph/view.rs:205-210`,
  `crates/leaven-engine/src/context/run_context.rs:640-652`,
  `docs/specs/first_two_subsystems.md:1689-1697`
- promised behavior: proposer-facing graph/evidence access is read-scoped, and
  evidence queries respect trust.
- actual behavior: `ProposalContext` exposes graph, read scope, budget, render,
  and materialize contexts. It can reveal `EvidenceRef`s through graph views,
  but only `RunContext::assessment_evidence()` can load payloads.
- why it matters: real reflection/proposal stages need trace and feedback
  payloads. Today they must receive ad hoc preloaded evidence or cannot use
  the engine proposer seam.
- correction direction: either add scoped evidence loading/materialization to
  `ProposalContext`, or define the contract that optimizers must lower complete
  evidence views into proposer requests before calling a proposer.

### L3-005: Hidden splits can be bypassed with explicit case IDs

- severity: high
- evidence: `crates/leaven-core/src/evaluation.rs:50-58`,
  `crates/leaven-engine/src/trust.rs:154-182`,
  `crates/leaven-engine/src/case_set.rs:64-70`,
  `docs/specs/eval_lowering_detail.md:675-678`,
  `docs/specs/eval_lowering_detail.md:752-757`
- promised behavior: validation/test partitions hidden by policy remain hidden
  from optimizer search even when requests are lowered through case sets.
- actual behavior: `TrustPolicy` checks partition-shaped requests, but
  `EvaluationSet::Cases(_)` is treated as non-hidden. `CaseSet::resolve()` can
  then accept explicit hidden case IDs without partition-membership checks.
- why it matters: optimizer authors can accidentally or deliberately use hidden
  validation/test cases by naming IDs directly.
- correction direction: add case-ID-to-partition trust checks after resolution,
  or refuse explicit case IDs when hidden partitions exist unless the caller
  has an explicit final-report trust mode.

### L3-006: Evaluation cache keys omit request semantics

- severity: high
- evidence: `crates/leaven-engine/src/cache.rs:47-59`,
  `crates/leaven-engine/src/context/run_context.rs:781-824`,
  `docs/specs/gepa_optimizer_surface.md:535-540`
- promised behavior: cache keys include evaluator fingerprint, resolved
  evaluation set, request shape, artifact identities, and semantics that affect
  assessment meaning.
- actual behavior: `EvaluationCacheKey` includes evaluator, policy, case-set
  version, case IDs, and candidate identities. It does not encode request kind,
  granularity, purpose, pair order, or assessment shape.
- why it matters: deterministic cache can reuse assessment IDs across
  semantically different requests, such as aggregate versus per-case or
  independent versus listwise over the same candidate/case set.
- correction direction: include resolved request kind, granularity, purpose if
  evaluator-visible, pair order/symmetry, and assessment shape in the cache key.

### L3-007: Scoring evaluator opts out of engine caching

- severity: medium
- evidence: `crates/leaven-run/src/evaluator.rs:61`
- promised behavior: high-level scoring should be able to use the engine's
  evaluation-cache semantics when the runner/scorer is deterministic or when a
  user explicitly chooses a cache policy.
- actual behavior: `ScoringEvaluator::cache_policy()` always returns
  `CachePolicy::Never`.
- why it matters: Layer 1 cannot inherit engine caching even for repeated
  deterministic scoring runs.
- correction direction: make evaluator cache policy configurable through the
  product builder and ensure cache keys capture the richer request semantics
  above before enabling reuse.
