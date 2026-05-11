## Boundary
This crate owns mechanical substrate shared by every Leaven layer: typed IDs,
content/blob refs, finite numeric wrappers, costs/budgets, fingerprints,
metadata bags, timestamps, and durable error records.

It must stay usable by storage, workspace, engine, adapters, and tooling
without importing optimizer algebra or runtime machinery.

## Route Here
- Mechanical identity and references: add new durable IDs, `StageId` variants,
  `BlobRef`, `EvidenceRef`, and content-address helpers here.
- Cost/accounting values: add dimensions, units, finite amount behavior, and
  `Metered<T>` plumbing here. Engine-owned ledgers belong in
  `leaven-engine`.
- Serialization-stable operational facts: metadata bags, retryability, error
  records, timestamps, and fingerprints belong here when they do not require
  artifact, graph, stage, workspace, or provider vocabulary.

## Local Helper Stack
- Use `Amount` for non-negative finite cost amounts; use `FiniteF64` when the
  value may be negative but must never be NaN or infinite. Do not hand-roll
  float checks in evidence/preference/population callers.
- Use UUID-backed IDs for run-graph records, `CaseId` for dense case indexes,
  and name-backed stage IDs for stable attribution. If a new ID names a graph
  record, mirror the existing UUID-newtype pattern in `src/ids.rs`.
- Feed every behavior-affecting knob into `FingerprintBuilder` in a stable
  order. Fingerprints identify behavior, not content; content identity stays in
  `ContentId` and artifact/cache traits.
- Keep large payloads out of `MetadataBag`; use `BlobRef` or `EvidenceRef` and
  store the bytes in the relevant store capability.

## Route Away
- Artifact identity, cache identity, apply errors, proposal provenance,
  evaluation requests, evidence markers, preference results, and problem
  associated types belong in `leaven-core`.
- Surface parts, addresses, selections, and surface fingerprints belong in
  `leaven-surface`; kernel fingerprints are only the byte primitive they wrap.
- Budget mutation, run accounting, trust/read scopes, graph records, and stage
  events belong in `leaven-engine`.
- Provider request IDs, API payload fields, workspace paths, and store backend
  keys belong in their adapter crates unless they are Leaven-wide mechanical
  IDs.

## Decision Cards
- when: adding a cache-key ingredient shared by evaluators, surfaces, renderers,
  or LM runtimes
  do: add the primitive here only if it is behavior-neutral and reusable across
    layers; otherwise put it in the owning capability crate
  preserve: the distinction between `Fingerprint` for behavior and `ContentId`
    for observable artifact content
  avoid: using `MetadataBag` as a cache-key side channel
  verify: run `cargo nextest run -p leaven-kernel --test identity_metadata`

- when: adding a new cost or budget value
  do: build it out of `Amount`, `Cost`, `CostAxis`, `Budget`, and
    `BudgetSnapshot`
  preserve: non-negative finite construction at the boundary
  avoid: moving engine ledger state, stop reasons, or callback accounting into
    kernel
  verify: run `cargo nextest run -p leaven-kernel --test cost_amount`

## Proof Anchors
- `src/lib.rs` is the crate map and public re-export list. Keep behavior in the
  owning module named by the concept.
- `tests/cost_amount.rs` proves non-negative finite costs, saturating
  combination, serde refusal, custom axes, and `Metered<T>` mapping.
- `tests/finite_f64.rs` proves generic finite-number semantics and metadata
  float construction.
- `tests/identity_metadata.rs` proves typed identity and metadata behavior.
- `cargo nextest run -p leaven-kernel` proves the mechanical substrate
  contract for local changes.

## Local Bait
- The name `Budget` here is a value object, not run spending state. Do not move
  `BudgetLedger`, `BudgetHandle`, stage spending, or callback accounting down
  from `leaven-engine`.
- `MetadataBag` is operational annotation storage, not a semantic escape hatch.
  If a fact changes proposal, evidence, artifact, or evaluation meaning, give
  it a typed home in the owning crate.
- `ContentId::zero()` is a testing/sentinel helper. It is not a valid shortcut
  for authored artifacts, cache keys, or placeholder derive implementations.
- IDs are allowed to be broad and cross-layer. That does not make this crate
  the home for the records, graph edges, provider payloads, or domain state
  those IDs point at.
