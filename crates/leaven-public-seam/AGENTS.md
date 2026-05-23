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

Crate-root exports for `CapabilityBudgetLedger`, `CapabilityBudgetUsage`, and
`CapabilityBudgetReservation` are advanced public seam contracts. They prove
aggregate capability-budget accounting against the locked capability document
only; they are not the engine budget ledger, ACP session accounting, provider
runtime metering, or durable spend persistence.

Crate-root export `CapabilityDelegation` is an advanced public seam contract.
It proves semantic parent-child capability attenuation and lineage facts at the
wire-document layer only; it is not a token minter, engine trust ledger,
transport session, ACP permission handler, or runtime delegation workflow.

Crate-root exports for `V1Scope`, `WorkerTransportKind`,
`WorkerTransportRequest`, and `AuthorizedWorkerTransport` are advanced public
seam contracts. They prove locked V1 transport-scope selection and MCP/watch/
legacy-worker exclusion only; they are not an ACP process implementation,
session lifecycle, authentication handshake, permission loop, or worker runtime.

Crate-root exports for `PlanDocument`, `PlanOperationKind`,
`PlanExecutionContext`, `PlanExecutionHost`, `PlanExecutionReport`,
`PlanLmCompleteRequest`, `PlanLmCompleteOutcome`,
`PlanEmitRunEventRequest`, and `PlanEmitRunEventOutcome` are advanced public
seam contracts. They prove active-schema Plan IR document validation and
Let/Call/Write family classification plus representative lowering/execution of
literal Let, `lm_complete` Call, and `emit_run_event` Write into a validated
Plan Result. They are not ACP delivery, provider runtime execution, cache
behavior, graph mutation authority, full Plan IR coverage, evaluator runtime
production, or runtime revision-read enforcement.

Crate-root export `PinnedDialectEvaluator` is an advanced public seam contract.
It proves deterministic parsing and replay for the V1 pinned wire
mini-languages: RFC 6901 JSON Pointer, the Leaven RFC 9535 JSONPath subset, and
`leaven.mustache.strict.v1`. It is not a full Plan IR executor, graph query
engine, template-extension host, or authorization layer.

Crate-root exports for `PlanResultDocument` and `Replayability` are advanced
public seam contracts. They prove active-schema plan-result envelope validation,
typed value/receipt/error/charge classification, operation receipt timing,
closed `PlanError` shape, and replayability roll-up at the wire-envelope layer
only; they are not plan-run production, evaluator execution, runtime receipt
production, graph mutation, or cache replay behavior.

Crate-root export `EvidenceEnvelopeDocument` is an advanced public seam
contract. It proves active-schema evidence-envelope visibility, data-class, and
source-receipt preservation plus target-derived data-class coverage at the
wire-envelope layer only; it is not evaluator evidence production, redaction
execution, receipt persistence, or data-class propagation through runtime stages.

Crate-root exports for `EvaluationJobDocument`, `EvaluationJobKind`, and
`EvaluationRequestReceiptDocument` are advanced public seam contracts. They
prove active-schema evaluation-job document validation plus semantic identity
checks for request id, candidate/case set, base revision, deadline, evaluator
id/fingerprint, capability fingerprint, request shape, and Plan Result
`request_evaluation` receipt binding. They are not proof that the runtime
evaluator path fully emits those jobs or receipts without the `leaven-run`
projection path.

Crate-root exports for `PublicOutputRecord`, `OutputRecordDocument`, and
`PublicBlobRef` are advanced public seam contracts. They prove reusable
`leaven-evidence` output records can be projected into the locked
`common.schema.json#/$defs/OutputRecord` wire shape with visibility,
data-classes, and blob audit metadata. They are not proof that evaluator
runtimes produce those records for every assessment shape.

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
  for typed Let/Call/Write documents, explicit consistency-mode bases, and
  rejection of unknown core/call/write kinds, top-level escape-hatch plan
  operations, mismatched `since_revision` event-source bases, and schema-valid
  placeholder `submit_assessments` score outputs before execution. Its
  `Score.output` checks require candidate/artifact data classes and a matching
  `evidence.public.summary` projection; this rejects candidate-labeled schema
  dummies but is still public-seam document validation, not runtime proof that a
  scorer assessed the true candidate output.
- `tests/plan_dialects.rs` proves pinned JSON Pointer, JSONPath, and strict
  Mustache dialects are parsed and replayed deterministically, and rejects
  unpinned path syntax, non-subset JSONPath filters/functions/scripts, non-
  strict template dialects, partials, unescaped templates, delimiter changes,
  and custom-filter syntax.
- `tests/evidence_envelope.rs` proves active-schema EvidenceEnvelope values
  preserve visibility projections, projection data classes, top-level target-
  derived data classes, and read/effect/write source receipt refs at the public-
  seam validation layer. It does not prove evaluator/evidence runtime production
  or public PlanResult projection.
- `tests/evaluation_job.rs` proves active-schema EvaluationJob values preserve
  evaluator/request/candidate/case/revision/deadline/capability identity for
  independent, pairwise, and listwise shapes, rejects missing deadline,
  evaluator fingerprint, capability fingerprint, unresolved case sets, and
  self-pairs, and validates `request_evaluation` Plan Result receipts against
  the job's candidate/case identity and audit hashes. It does not prove the
  runtime evaluator creates those jobs or emits evaluation request receipts
  unless paired with the `leaven-run` runtime projection tests.
- `tests/output_record.rs` proves reusable `leaven-evidence` output records
  project into the locked public-seam OutputRecord wire shape with visibility,
  data classes, non-placeholder inline output, and public blob audit metadata.
  It does not prove evaluator runtime production or pairwise/listwise
  assessment behavior.
- `tests/plan_result.rs` proves active-schema Plan Result envelopes carry typed
  success and failure values, query/call/write audit receipts, errors, charges,
  capability and policy fingerprints, receipt timing, data classes, and closed
  `PlanError` values at the public-seam validation layer. It does not prove an
  engine/run producer emits those envelopes.
- `tests/plan_result_replayability.rs` proves assessment batch result values
  preserve per-assessment replayability and that plan-level replayability is a
  roll-up summary, not a single boolean or override.
- `tests/plan_result_evidence.rs` proves Plan Result values semantically inspect
  nested `Score.output` and `EvidenceEnvelope` payloads, require value data
  classes to cover score outputs and evidence projections, and reject evidence
  source receipt refs that are missing or categorized as the wrong
  query/call/write receipt kind. It does not prove evaluator evidence
  production, runtime receipt persistence, or full data-class propagation across
  query/call/write execution.
- `tests/capability_document.rs` proves opaque token handles resolve to
  structured capability documents and reject bare, missing, expired, revoked,
  or binding-mismatched tokens. It also proves grant-envelope authorization and
  denials for action, resource, partition, case-field, schema, surface,
  data-class, and per-grant limit constraints. Aggregate budget tests prove
  cross-grant total, role-specific, and concurrent-call limits against the
  public capability document. Delegation tests prove valid child capabilities
  record parent lineage and cannot widen action, resource, budget, data-class,
  schema, expiry, binding, or delegation-policy authority.
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
