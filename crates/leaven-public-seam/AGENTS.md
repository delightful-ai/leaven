## Boundary

This crate owns the locked V1 public seam wire contract for external-language
workers: manifest inventory, active schema/profile loading, contract
fingerprints, conformance-matrix harness data, and deferred-marker enforcement.
It also owns the wire-level capability document shape, opaque-token resolution
guardrails, and grant-envelope authorization checks needed before runtime
owners spend budgets or execute effects.

It is not a worker runtime, graph mutation layer, provider adapter, or schema
code generator. Aggregate budget spending, data-class propagation across
results, and runtime behavior must land in the owning engine, agent, workspace,
LM, evaluator, or run crates and be exercised through this seam before any
conformance row can claim integrated behavior.

## Route Here

- Loading only `docs/specs/public-seam-v1` or artifacts embedded from that
  active package.
- Schema compilation and example validation against the active manifest.
- Capability documents resolved from opaque handles, including subject
  fingerprint, binding, expiry, revocation, renewal, grants, and aggregate
  budget truth.
- Grant-envelope authorization for action, resource selectors, case fields,
  partitions, schemas, surface fingerprints, data classes, and per-grant limits.
- RFC 8785 JCS plus SHA-256 schema fingerprint values such as
  `fp_schema_sha256_*`.
- Conformance-matrix row parsing, uniqueness checks, spec-reference checks,
  notes-denominator parsing, and row evidence/status helpers.
- V1 hard-cutover markers: MCP is not V1, `watch.v1` runtime behavior is
  deferred, and `worker_protocol.v1` is deprecated in favor of the ACP profile.

## Route Away

- Cold optimizer vocabulary stays in `leaven-core`.
- Mechanical BLAKE3 behavior fingerprints stay in `leaven-kernel`; this crate's
  schema fingerprints are public-seam wire identifiers, not cache behavior
  fingerprints.
- Graph mutation remains private to `leaven-engine` through `RunContext`.
- ACP process/session behavior belongs in the future worker transport owner.
- Provider/runtime lowering belongs in `leaven-lm*`, `leaven-agent*`, and
  workspace crates.

## Public Maturity

This crate is an advanced public contract for implementers of the external
worker seam. It is not routed through `leaven::prelude`, default umbrella
features, or examples as ordinary product proof in this initial slice.

Crate-root exports for `ConformanceTest*`, `ConformanceRow`, and matrix/status
types are advanced harness contracts. They are evidence plumbing for the locked
package, not proof that the runtime rows they describe have product behavior.

Crate-root exports for `CapabilityDocument`, `CapabilityRegistry`, and
`CapabilityError` are advanced public seam contracts. They prove token-to-
document authority resolution only; they are not ACP authentication or
budget-ledger product routes.

Crate-root exports for `CapabilityGrantRequest`, `CapabilityLimitUsage`,
`AuthorizedGrant`, `CapabilityDenial`, and `CapabilityDenialKind` are advanced
public seam contracts. They prove grant-envelope authorization only; they are
not aggregate budget ledgers, delegation engines, ACP permission handlers, or
runtime effect executors.

Crate-root export `CapabilityDelegation` is an advanced public seam contract.
It proves semantic parent-child capability attenuation and lineage facts at the
wire-document layer only; it is not a token minter, engine trust ledger,
transport session, ACP permission handler, or runtime delegation workflow.

Crate-root exports for `V1Scope`, `WorkerTransportKind`,
`WorkerTransportRequest`, and `AuthorizedWorkerTransport` are advanced public
seam contracts. They prove locked V1 transport-scope selection and MCP/watch/
legacy-worker exclusion only; they are not an ACP process implementation,
session lifecycle, authentication handshake, permission loop, or worker runtime.

Crate-root exports for `PlanDocument` and `PlanOperationKind` are advanced
public seam contracts. They prove active-schema Plan IR document validation and
Let/Call/Write family classification only; they are not plan execution, lowering
to engine operations, cache behavior, graph mutation authority, or runtime
consistency enforcement.

Crate-root export `PinnedDialectEvaluator` is an advanced public seam contract.
It proves deterministic parsing and replay for the V1 pinned wire
mini-languages: RFC 6901 JSON Pointer, the Leaven RFC 9535 JSONPath subset, and
`leaven.mustache.strict.v1`. It is not a full Plan IR executor, graph query
engine, template-extension host, or authorization layer.

Crate-root export `DeferredWatchReplacement` is an advanced public seam
contract. It proves that the V1 deferred watch marker can route only to a finite
`consistency.since_revision` event-diff Plan IR document; it is not watch
subscription delivery, streaming, cursor acknowledgement, lifecycle,
backpressure, or runtime watch support.

## Proof Anchors

- `tests/contract_package.rs` proves active package authority, manifest
  inventory, schema compilation, schema fingerprinting, matrix row structure,
  notes-denominator mapping, fake-closeout rejection, deferred markers, and
  locked ACP-profile transport-scope refusal of MCP, legacy worker protocol,
  and watch runtime requests. It also proves the deferred watch marker routes to
  a finite `since_revision` event-diff plan instead of watch runtime behavior.
- `tests/plan_document.rs` proves schema-backed Plan IR document classification
  for typed Let/Call/Write documents and rejection of unknown core, call, write,
  and top-level escape-hatch plan operations before execution.
- `tests/plan_dialects.rs` proves pinned JSON Pointer, JSONPath, and strict
  Mustache dialects are parsed and replayed deterministically, and rejects
  unpinned path syntax, non-subset JSONPath filters/functions/scripts, non-
  strict template dialects, partials, unescaped templates, delimiter changes,
  and custom-filter syntax.
- `tests/capability_document.rs` proves opaque token handles resolve to
  structured capability documents and reject bare, missing, expired, revoked,
  or binding-mismatched tokens. It also proves grant-envelope authorization and
  denials for action, resource, partition, case-field, schema, surface,
  data-class, and per-grant limit constraints. Delegation tests prove valid
  child capabilities record parent lineage and cannot widen action, resource,
  budget, data-class, schema, expiry, binding, or delegation-policy authority.
- `cargo test -p leaven-public-seam --test contract_package` is the focused
  proof for this crate.
- `cargo test -p leaven --test topology_contract` must pass when this crate is
  added or its dependency edges change.

## Hazards

- Do not accept archived draft directories, downloaded zips, or MCP-over-ACP
  draft payloads as current V1 input.
- Do not mark conformance rows proven from schema compilation alone unless the
  row explicitly says `shape_only`.
- Do not add generated structs that round-trip JSON but are never executable and
  call that the seam.
