# Leaven Testing Contract

Leaven uses tests to collapse the remaining implementation space after the
type, trait, and error surfaces are honest. Every test must name a real claim
and kill a plausible wrong implementation.

## Canonical Check

Run the full local gate before claiming behavior is complete:

```bash
just check
```

`just check` runs formatting, the production line-count lint, clippy with
workspace targets, the nextest workspace suite, doctests, and the coverage
summary. Use narrower recipes only while iterating:

```bash
just lint
just test
just test-one <nextest selector>
just test-stress 20 <nextest selector>
just coverage
```

## Runtime SLA

The canonical full test suite has a hard wall-clock SLA:

```text
just test must finish in <30s
```

`just test` enforces this directly. The SLA covers the nextest workspace suite
and workspace doctests together. If the suite crosses the line, do not add a
second slow lane; reduce fixture cost, property-test case count, setup work, or
assertion altitude until the default suite is back under the SLA.

Coverage is a ratchet. Raise `coverage_line_floor` in the root `Justfile` when
coverage improves; do not lower it to land weaker tests.

The v0.2.1b topology cutover adds many spec-listed crate skeletons and
trait-only surfaces whose job is to enforce dependency direction before their
behavior lands. The coverage gate now keeps a `98.0` line floor and includes
the behavior-bearing P1 engine/context/cache/budget/trust/graph files plus
scalar evidence, scalar preference, keep-best population, and inline evidence
storage. Empty adapters, trait-only stage surfaces, map-only binaries, and
unimplemented skeleton crates stay out of the denominator until they gain real
runtime behavior. When a skeleton crate gains runtime logic, remove it from
`coverage_ignore` in the same change that adds its contract tests.

## Test Shapes

Use the narrowest layer that proves the claim.

- **Law tests** prove algebraic invariants over a space of inputs. Prefer
  `proptest` for type, graph, budget, cache-key, and resolution laws.
- **Example tests** pin a canonical input/output or error shape.
- **Scenario tests** drive a public surface across components, such as
  `RunContext` proposal/apply/evaluate flows.
- **Regression tests** fossilize a previously broken combination until the
  underlying rule is understood and moved into a law or example.

## Layout

- Private helper and invariant tests live beside the production code under
  `#[cfg(test)] mod tests` when they need private implementation access.
- Public crate behavior lives in integration tests under
  `crates/<crate>/tests/`.
- Shared integration-test harness code lives in `tests/support/` once two or
  more files need it. Keep it typed and explicit; do not hide behavior in
  stringly setup.
- Cross-cutting guardrails live in contract tests named for the promise they
  defend, for example `crate_boundary_contract.rs`.
- Property tests are named for laws, not implementation details, for example
  `property_laws.rs`.

## Current Suites

- `crates/leaven-engine/tests/graph_surface.rs`: scenario and example coverage
  for proposal application, lineage, informed-by edges, and failed apply
  attempts through `RunContext`, plus an append-only graph law test.
- `crates/leaven-engine/tests/context_services.rs`: proposal/evaluation
  context scenarios for budget charging, error recording, cache policy,
  evidence references, assessment visibility, read scopes, and graph-backed
  evidence lookup.
- `crates/leaven-engine/tests/budget_laws.rs`: budget axis and sub-stage
  charging laws, including typed refusal of invalid seconds amounts.
- `crates/leaven-kernel/tests/cost_amount.rs`: property and regression coverage
  for finite, non-negative `Amount` construction, serde round trips, and
  saturating cost combination.
- `crates/leaven-engine/tests/case_set_resolution.rs`: evaluation-set
  resolution examples for partitions, explicit cases, set combinators,
  deterministic sampling, and typed refusal.
- `crates/leaven-engine/tests/trust_policy.rs`: read-scope and actor-policy
  examples for hidden partitions.
- `crates/leaven-engine/tests/engine_loop.rs`: engine run-loop scenarios for
  continuation, optimizer completion, and optimizer error closeout.
- `crates/leaven-engine/tests/stage_trait_contracts.rs`: static-to-dynamic
  stage adapter contracts for proposer, evaluator, preference, and stopper
  traits.
- `crates/leaven-derive/tests/derive_macros.rs`: `trybuild` contract proving
  reserved derive macros fail explicitly until their real codegen contracts
  land.
- `crates/leaven/tests/scalar_keep_best.rs`: P1 end-to-end scenario from seed
  proposal through evaluation, inline evidence storage, keep-best population,
  callbacks, and best-candidate result.
- `crates/leaven/tests/topology_contract.rs`: guardrails for the full corrected
  v0.2.1b workspace member list, `src/lib.rs` skeleton presence,
  Leaven-to-Leaven dependency DAG, and cold-core leak boundaries.
- `crates/leaven-evidence/tests/scalar.rs`,
  `crates/leaven-preference/tests/scalar.rs`,
  `crates/leaven-population/tests/keep_best.rs`, and
  `crates/leaven-store-inline/tests/evidence.rs`: finite scalar evidence,
  scalar preference, keep-best, and inline store behavior now covered by the
  canonical coverage gate.

## Review Rules

- A test without a claim is noise.
- A test that only passes through ceremony belongs at a lower layer or should be
  deleted.
- Public holes for tests are forbidden. Use public behavior, crate-private unit
  tests, or a typed test harness.
- Process environment mutation must be isolated behind a helper that restores
  state. Prefer explicit typed setup.
