# Layer 3 RunContext And Graph

Status: active findings recorded.

Layer 3 covers optimizer authors and internal engine users who need the graph,
context, budget, evidence, and mutation APIs to be honest and sufficient.

## Findings

### L3-008: `RunContext` is the right authority, but nearby public holes weaken it

- severity: high
- evidence: `crates/leaven-engine/src/context/run_context.rs:191-208`,
  `crates/leaven-engine/src/context/run_context.rs:286-310`,
  `crates/leaven-engine/src/context/run_context.rs:640-653`,
  repo `AGENTS.md` global invariant: "`RunContext` is the public mutation path
  into `RunGraph`"
- promised behavior: graph mutation and evidence reads that affect optimizer
  behavior route through `RunContext`, preserving trust, budget, event, cache,
  and durable-record invariants.
- actual behavior: `RunContext` has the correct high-level proposal and
  evidence methods, but public raw context factories and missing proposer
  evidence access encourage users to route around it.
- why it matters: the engine can be correct in the center while public seams
  train optimizer authors to bypass the center.
- correction direction: keep `RunContext` as the only public finalizing stage
  path. Anything lower-level should be crate-private, explicit test support, or
  named as a non-finalizing internal context.

### L3-009: Tests assert private dispatch paths more than public invariants

- severity: medium
- evidence: `crates/leaven-engine/tests/stage_trait_contracts.rs:24-92`,
  `crates/leaven-engine/tests/materializer_contract.rs:205-213`
- promised behavior: tests should assert public/capability behavior unless an
  invariant is genuinely private.
- actual behavior: some tests call raw stage contexts directly and therefore
  validate object-safety/dispatch without proving the public finalization
  behavior that users depend on.
- why it matters: tests can pass while the product path is still incomplete or
  easy to bypass.
- correction direction: move raw-context tests into crate-local private tests
  and add public `RunContext` contract tests for budget charging, events,
  evidence storage, and cache behavior.
