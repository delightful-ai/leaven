# Fix Priority Map

Status: active findings recorded.

This file orders correction work by product truth: remove false-positive proof
paths first, then restore power-user seams, then clean naming and topology.

## Priorities

### P0: Stop False Product Proof

- Replace the fixed-edit AIME "GEPA" proof with a real reflective mutation path
  or mark it as a deterministic fixture demo.
- Remove the Python OpenAI bypass from the live AIME proof path.
- Split proxy/demo examples from product capability gates.

Why first: otherwise every later implementation can keep passing by proving the
wrong thing.

### P1: Restore The Ordinary User Surface

- Hard-cut `leaven-run` runner/scorer to async-capable contracts.
- Implement rich `Score`, `ScoreContext`, trace, attachment, and metered score
  behavior.
- Add stable case identity for train / validation / test.
- Wire solver/reflector runtime and cache policy through the run builder.
- Split ordinary prelude from engine-author imports.

Why second: this is the surface a user must trust before the optimizer library
is usable off the shelf.

### P2: Make GEPA Customization Real

- Replace `SurfaceProposer` / fixed `ReflectiveMutation` with an async
  trace/evidence-aware reflector/proposer contract.
- Add builder slots for parent selection, part selection, batch sampling,
  acceptance, validation policy, population/frontier, merge, and stopping.
- Preserve feedback/evidence refs until reflection.
- Replace scalar-only `Gate` with evidence/preference-aware acceptance.

Why third: power users need swappability, but it must build on the restored
ordinary evidence and runtime contracts.

### P3: Seal Engine Invariant Bypasses

- Make raw stage contexts private/test-only or clearly non-finalizing.
- Add `RunContext` finalizers for render/materialize.
- Add scoped evidence access to proposer context or require complete
  evidence-view requests.
- Fix trust checks for explicit case IDs.
- Strengthen evaluation cache keys.

Why fourth: these are correctness foundations for optimizer authors and for
GEPA internals.

### P4: Clean Topology And Placeholder Exports

- Delete or hard-cut `crates/leaven-dsrs` into the workspace.
- Remove derive defaults until derives work.
- Remove public placeholder exports from standard/provider/backend facades.
- Extend topology tests to reject orphan dirs and unapproved public stubs.
- Make `pub mod` file-layout exports private unless intentionally public.

Why fifth: this prevents future drift and makes the crate graph tell the truth.
