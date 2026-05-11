# Root Cause Map

Status: active findings recorded.

This file maps repeated failure patterns across findings so fixes can address
the root design mismatch instead of one-off symptoms.

## Patterns

### R-001: Proxy Proof Replaces Product Proof

Symptoms:

- AIME score movement comes from a fixed prompt replacement, not real
  reflection.
- Live AIME shells out to Python instead of the Leaven LM/provider/cache
  surface.
- Coverage can include demo/proxy examples without proving the product
  capability.

Root diagnosis: examples and tests are allowed to prove nearby behavior without
requiring the same public surface a user would run.

Fix principle: acceptance examples must use the public surface under test.
Fixtures can exist, but their names and gates must say fixture/demo, not
optimizer proof.

### R-002: Names Arrive Before Contracts

Symptoms:

- `ReflectiveMutation`, `GepaConfig`, `MergeScheduler`, evidence types,
  renderers, derives, and provider crates are public before they have real
  behavior.
- Standard facades re-export empty unit structs.

Root diagnosis: topology/scaffolding was treated as product surface. The crate
graph is useful, but public names without laws are false affordances.

Fix principle: public names must either be real contracts or explicitly marked
scaffolding outside ordinary import paths.

### R-003: Engine Invariants Exist But Are Easy To Bypass

Symptoms:

- Raw public stage contexts bypass `RunContext` finalization.
- GEPA records proposal batches manually.
- Render/materialize stages return `Metered` but lack public finalizers.

Root diagnosis: internal plumbing escaped as power-user API before the
finalizing public path was complete.

Fix principle: `RunContext` remains the public mutation/finalization authority.
Raw contexts are private, test-only, or explicitly non-finalizing.

### R-004: Rich Evidence Is Collapsed Too Early

Symptoms:

- `Score` is `f64 + String + Vec<(String, String)>`.
- GEPA converts feedback/evidence into scalar casewise summaries before
  reflection.
- `ProposalContext` cannot load scoped evidence payloads.

Root diagnosis: scalar score plumbing landed before the feedback/trace contract
was preserved end-to-end.

Fix principle: scores, traces, attachments, evidence refs, and rendered views
stay typed until the stage that intentionally consumes or projects them.

### R-005: Layer Boundaries Are Named But Not Enforced In Import Surfaces

Symptoms:

- ordinary prelude exports engine internals;
- Layer 2 builder hides promised slots;
- topology tests do not reject public placeholders.

Root diagnosis: the conceptual layers exist in specs, but the Rust import
surface and tests do not enforce those layers.

Fix principle: separate ordinary, customizer, and engine-author surfaces in
exports and contract tests.
