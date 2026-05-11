# Audit Plan

This is the continuation plan after the already-known findings were made
durable.

## Audit Lenses

The audit is organized by user-visible layer first, crate second.

### Layer 1: Ordinary User

Question: can a user run a real optimizer over a real LM/agent/program without
learning internals or relying on examples that fake the hard part?

Scope:

- `crates/leaven`
- `crates/leaven-run`
- public examples
- LM/runtime/cache ergonomics
- reports/results

### Layer 2: GEPA Customizer

Question: can a power user swap GEPA strategies without forking GEPA or losing
access to necessary trace/evidence context?

Scope:

- `crates/leaven-gepa`
- `crates/leaven-surface`
- `crates/leaven-population`
- `crates/leaven-preference`
- `crates/leaven-render`

### Layer 3: Optimizer Author / Engine User

Question: can an optimizer author use the engine primitives directly and
correctly, with no hidden missing accessors or duplicated local shadows?

Scope:

- `crates/leaven-core`
- `crates/leaven-engine`
- `crates/leaven-store`
- `crates/leaven-evidence`
- `crates/leaven-eval`

### Cross-Cutting Crate/Stub Inventory

Question: which crates, modules, examples, or public exports are scaffolding
but named/presented like finished behavior?

Scope:

- all `crates/*`
- all examples
- root and crate `Cargo.toml`
- docs/specs crate graph
- public re-exports
- `placeholder`, `skeleton`, `TODO`, `unimplemented`, fixed fixture names

## Output Documents To Fill

Planned docs:

- `inventory/crate-inventory.md`
- `inventory/public-api-ledger.md`
- `inventory/stub-placeholder-ledger.md`
- `surfaces/layer-1-user/public-api-ledger.md`
- `surfaces/layer-1-user/examples-and-end-to-end-proof.md`
- `surfaces/layer-1-user/evaluation-datasets-results.md`
- `surfaces/layer-2-gepa-customizer/strategy-slots.md`
- `surfaces/layer-2-gepa-customizer/reflection-and-proposal.md`
- `surfaces/layer-2-gepa-customizer/evidence-trace-selection.md`
- `internals/layer-3-engine-author/run-context-and-graph.md`
- `internals/layer-3-engine-author/stage-contexts.md`
- `internals/layer-3-engine-author/evidence-trust-budget-cache.md`
- `cross-cutting/lm-and-cache-surface.md`
- `cross-cutting/topology-and-crate-graph.md`
- `cross-cutting/root-cause-map.md`
- `cross-cutting/fix-priority-map.md`

## Rules For Findings

Each finding should include:

- severity;
- path and line reference;
- which layer it harms;
- what the API appears to promise;
- what actually happens;
- why this blocks real use;
- required correction direction.

No finding should be based only on vibes. If the smell is naming, cite the name
and the implementation behind it.
