# Layer 3 Stage Contexts

Status: active findings recorded.

This file audits proposer, evaluator, renderer, selector, and related stage
contexts for missing accessors, sync-only seams, duplicated local shadows, or
test-only public holes.

## Findings

### L3-001: Public raw stage contexts bypass `RunContext` finalization

- severity: high
- evidence: `docs/specs/first_two_subsystems.md:1503-1505`,
  `docs/specs/first_two_subsystems.md:1538-1542`,
  `crates/leaven-engine/src/context/run_context.rs:286-310`,
  `crates/leaven-engine/tests/stage_trait_contracts.rs:24-92`
- promised behavior: `RunContext` is the public path that records proposals
  and evaluations, charges budget, stores evidence, applies cache, emits
  events, and enforces trust/read scope.
- actual behavior: public `proposal_context()` and `evaluation_context()`
  factories let callers invoke stages directly. Tests use those factories to
  bypass proposal/evaluation recording, evidence storage, cache insertion, and
  final budget charging.
- why it matters: optimizer authors see a legitimate-looking public hook that
  skips the invariants the engine exists to preserve.
- correction direction: make raw context factories internal/test-support only,
  or expose a wrapper that always finalizes metered stage output through
  `RunContext`.

### L3-002: Renderer/materializer stages lack public finalizers

- severity: medium
- evidence: `docs/specs/initial_library.md:1876-1883`,
  `docs/specs/first_two_subsystems.md:1641-1642`,
  `crates/leaven-engine/src/stage/renderer.rs:8-25`,
  `crates/leaven-engine/src/context/run_context.rs:313-325`,
  `crates/leaven-engine/tests/materializer_contract.rs:40-45`
- promised behavior: render/materialize are costful stage work that should
  route through `RunContext::charge` and event normalization.
- actual behavior: renderer/materializer traits return `Metered`, but there is
  no first-class `RunContext::render` or `RunContext::materialize_into`
  finalizer. Tests call materializers directly and ignore returned cost.
- why it matters: optimizer authors can do real async workspace/render work
  without budget events or checkpoint boundaries.
- correction direction: add `RunContext` render/materialize finalizers and keep
  raw contexts as internal plumbing.

### L3-003: GEPA uses manual proposal recording instead of the engine stage path

- severity: high
- evidence: `crates/leaven-gepa/src/optimizer.rs:536-594`,
  `crates/leaven-engine/src/context/run_context.rs:191-208`
- promised behavior: optimizer implementations should use the same proposal
  path that engine users rely on, so proposal provenance, budget, events, and
  graph mutation stay centralized.
- actual behavior: GEPA creates a proposal batch itself and calls
  `record_proposal_batch` / `apply_proposal_batch` rather than using
  `RunContext::propose`.
- why it matters: the implementation can drift from the public engine
  contract, and it already has drifted on proposal context and reflection.
- correction direction: make GEPA proposal generation a normal engine proposer
  call or add a GEPA-specific `RunContext` helper that preserves the same
  invariants.
