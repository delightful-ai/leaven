# Layer 2 GEPA Customizer Surface Audit Seed

Layer 2 users customize GEPA. They should touch algorithm slots such as
surface, parent selection, part selection, batch sampling, reflection/proposal,
acceptance, population, validation cadence, merge, and stopping.

They should not need to fork the engine or manually recover evidence from graph
internals to implement a real GEPA variant.

## Already Found Problems

### Reflection Slot Cannot See Reflection Inputs

`SurfaceProposer<A, S>` only gets:

```rust
artifact
surface
part
```

That is not a reflective mutation contract. A reflective mutation slot needs
the selected parent, selected surface part, current part text, evaluation
feedback, traces, scores, relevant graph/lineage context, objective/background,
and budget/cost surface.

### Reflection Slot Is Sync

`SurfaceProposer::propose_edit` is synchronous. Real reflectors are commonly
LMs, local agents, remote agents, or materialized workspace agents. Those are
async by nature.

### `ReflectiveMutation` Is Misnamed

The current `ReflectiveMutation` is a fixed edit fixture. That name must not be
the public name for a canned edit. It creates a false claim that a real GEPA
reflection stage is present.

### GEPA Does Not Use Engine `Proposer<P>`

The engine proposer seam already gives graph-aware context and stage-scoped
budget accounting. GEPA uses `ctx.record_proposal_batch(...)` directly after a
GEPA-local fixed/narrow reflection call.

This bypass is the main architectural drift.

## Canonical Layer 2 Audit Docs

- `root-cause-map.md`: Layer 2 GEPA root causes only.
- `fix-priority-map.md`: ordered GEPA/customizer fixes and proof gates.
- `vision-comparison.md`: original GEPA vision compared with current repo state.
- `surface-requirements.md`: exact GEPA customizer slot, request/response,
  error, invariant, state, and proof contract.

## Layer 2 Audit Questions For The Broader Pass

- Which GEPA strategy slots are real, swappable, and tested through behavior?
- Which slots are placeholders with production-looking names?
- Does each strategy slot receive the exact information it needs, no less and
  no hidden globals?
- Can the reflection slot be backed by either an LM or an agent without a new
  trait?
- Does GEPA use the engine's proposal/evidence/budget/event primitives instead
  of reimplementing local shadows?
- Are merge, validation cadence, and batch sampler names backed by behavior or
  just future-shaped structs?
