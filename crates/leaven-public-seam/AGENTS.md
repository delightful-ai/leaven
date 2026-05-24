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
runtime effect executors. Their request dimensions include action, resource
selectors, case fields, partitions, data classes, purposes, model roles, model
ids, workspace operations, allowed command names, schema fingerprints, surface
fingerprints, and per-grant limits.

Crate-root exports for `CapabilityBudgetLedger`, `CapabilityBudgetUsage`, and
`CapabilityBudgetReservation` are advanced public seam contracts. They prove
aggregate capability-budget accounting against the locked capability document
and can lower aggregate limits/usages into `leaven-kernel` `Budget`/`Cost`
primitives for the engine ledger. Delegated runtime costs must be projected
through the parent capability and charged against the parent's/shared engine
ledger; child capabilities do not mint independent aggregate budget state.
Runtime projection rejects integer budget values above the current exact
kernel amount boundary. These exports are not the engine budget ledger, ACP
session accounting, provider runtime metering, or durable spend persistence.

Crate-root export `CapabilityDelegation` is an advanced public seam contract.
It proves semantic parent-child capability attenuation and lineage facts at the
wire-document layer only; it is not a token minter, engine trust ledger,
transport session, ACP permission handler, or runtime delegation workflow.

Crate-root exports for `V1Scope`, `WorkerTransportKind`,
`WorkerTransportRequest`, and `AuthorizedWorkerTransport` are advanced public
seam contracts. They prove locked V1 transport-scope selection and MCP/watch/
legacy-worker exclusion only; they are not an ACP process implementation,
session lifecycle, authentication handshake, permission loop, or worker runtime.

Crate-root exports for `AcpProfileDocument`, `AcpExtensionMethod`,
`AcpAuthenticateRequest`, `AcpAuthenticatedSession`, `AcpPermissionRequest`,
`AcpPermissionDecision`, and `AcpExtensionResultDocument` are advanced public
seam contracts. They prove locked Leaven ACP profile semantics, including the
exact V1 extension-method set; authenticate resolution from opaque capability
tokens through the capability registry; authenticated-session binding for
programmatic permission decisions against capability grants; typed denial
envelopes; locked Plan IR/Plan Result schema binding for profile methods; and
schema-backed, hash-bound extension-result envelopes at the wire-contract layer
only. Generic ACP `extension` primaries are checked against the locked schema
branch and ACP envelope fields, while concrete PlanResult value kinds still run
the full PlanResult semantic validator. They are not an ACP process
implementation, session lifecycle, transport backpressure loop, engine-client
runtime, worker-agent runtime, provider call, or graph mutation route.

Crate-root exports for `PlanDocument`, `PlanOperationKind`,
`PlanExecutionContext`, `PlanExecutionHost`, `PlanExecutionReport`,
`PlanGraphReadScope`, `PlanGraphQueryRequest`, `PlanGraphQueryOutcome`,
`PlanCaseQueryRequest`, `PlanCaseQueryOutcome`, `PlanLmCompleteRequest`,
`PlanLmCompleteOutcome`, `PlanAgentRunRequest`, `PlanAgentRunOutcome`,
`PlanSandboxExecRequest`, `PlanSandboxExecOutcome`,
`PlanWorkspaceMaterializeRequest`, `PlanWorkspaceMaterializeOutcome`,
`PlanWorkspaceReleaseRequest`, `PlanWorkspaceReleaseOutcome`,
`PlanEmitRunEventRequest`, `PlanEmitRunEventOutcome`, `CallAuthorityReport`, and
`ProposalAuthorityReport` are advanced public seam contracts. They prove
active-schema Plan IR document validation and Let/Call/Write family
classification plus representative lowering/execution of literal Let,
`graph_query` reads, `case_query.load` reads, `lm_complete` Call, and
`emit_run_event` Write into a validated Plan Result with query/call/write
receipts. The `PlanLmCompleteRequest::to_lm_request` route lowers
schema-valid `lm_complete` calls into provider-neutral `leaven-lm` vocabulary
while preserving developer/user/tool messages, tool-result ids, tool
definitions, model role, sampling stop sequences, provider hints, final-message
output, and JSON-schema output. JSON-schema LM outputs must return a parsed
Plan Result payload, and successful call-result validation requires an
`lm_complete` result to be an `lm_response` value carrying the matching call
receipt; extension/multimodal content is rejected rather than silently
downgraded to text. It still requires a concrete model before provider
execution and is not provider runtime execution, streaming, ACP delivery, or
full `ps1.lm.contract` closeout. The `PlanAgentRunRequest` and
`PlanSandboxExecRequest` routes lower schema-valid `agent_run` and
`sandbox_exec` calls into provider-neutral `leaven-agent::AgentRunRequest` and
backend-neutral `leaven-workspace::Command` primitives, and the representative
harness can emit typed `agent_session` and `sandbox_exec` Plan Result values
with call receipts. JSON-schema agent outputs must return a parsed Plan Result
payload, and successful call-result validation requires `agent_run` and
`sandbox_exec` result values to carry the matching call receipt and expected
value kind. `sandbox_exec` with `stream_policy: blob_refs_only` must return
stdout/stderr blob refs, and sandbox outcomes can carry captured output-file
blob refs. Both `agent_run` and `sandbox_exec` require a live unreleased,
materialization-proven `workspace_handle` dependency before host execution, so
host paths, bare workspace ids, and literal forged handles cannot satisfy
either route. These routes are not agent provider execution, sandbox backend
execution, ACP delivery, proposal parsing, `stream_updates` transport delivery,
or full agent/sandbox row closeout. The
`PlanWorkspaceMaterializeRequest` and
`PlanWorkspaceReleaseRequest` routes lower schema-valid workspace lifecycle
calls into typed public-seam requests and emit `workspace_handle` Plan Result
values with call receipts; `workspace_materialize` validates host-returned
workspace ids and lifetime echoes before binding a handle, and
`workspace_release` validates the requested WorkspaceRef against live
materialization-proven `workspace_handle` dependency values before the host can
perform release, rejects already released handles, and still refuses literal
forgeries and host filesystem paths as workspace handles. `PlanWorkspaceQueryRequest` routes
representative schema-valid `workspace_query` reads through typed host requests,
requires a live materialization-proven `workspace_handle` dependency before the
host can read, and emits typed `workspace_file`, `workspace_listing`,
`workspace_snapshot`, or `workspace_diff` values with query receipts. `stat`
projects as a `workspace_listing`, `digest` as a `workspace_snapshot`, and
`git_log` as a `workspace_diff` because the locked result schema has those
workspace-read value families rather than separate stat/digest/log value kinds.
Within that broad family mapping, `stat` still binds the result to the
requested path and `digest` still binds the result to the requested algorithm
and workspace id. `git_log` remains a broad `workspace_diff`-family projection,
not a parsed commit-log surface.
It is not workspace backend execution and still leaves full artifact/snapshot
backend proof pending, so it is not full workspace row closeout. The
`PublicSeamPackage::execute_plan_document_with_capability` route
checks Plan call authority against the supplied capability before host effects
can run and requires capability-authorized evaluator request scope before a
`case_query.load` host read can run. The
`PublicSeamPackage::validate_plan_execution_result` route additionally proves
representative query/call/write receipt hashes can be checked against the Plan
IR and execution-context preimages instead of accepted as decorative ids, and
that representative workspace-query receipt validation rejects literal forged
workspace handles rather than treating hash-correct values as materialized
provenance. They also prove the
representative harness distinguishes `latest_at_start`, `at_revision`, and
`since_revision` graph-read scopes plus `execute`, `dry_run`, `require_cached`,
and `replay` mode side-effect surfaces: dry-run validates without host effects,
require-cached refuses cache misses without live provider calls, and replay
loads supplied receipts without live call/write host effects. The call-authority
route checks plan Call `input_classes` against call-local and
capability-declared forbidden classes before execution, and refuses reflector
LM calls that carry `case.target` input classes even when a grant is too broad.
The proposal authority
route checks `submit_proposal_batch` and `apply_proposal_batch` writes against
capability-granted effects, surfaces, schemas, and apply permission. They are
not ACP delivery, provider runtime execution, general cache
backend behavior, graph mutation authority, full Plan IR coverage, evaluator
runtime production, or engine RunGraph revision reads.

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
wire-envelope layer only. Target-derived evidence must also carry a read receipt
so target-derived facts cannot pass as unreceipted policy metadata. This is not
evaluator evidence production, redaction execution, receipt persistence, or
data-class propagation through runtime stages.

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

Crate-root exports for `StagePayloadDocument`, `StagePayloadRole`, and
`StageProposalEffect` are advanced public seam contracts. They prove
active-schema stage-payload validation for role-specific reflector,
reflection-result, proposer, runner, scorer, judge, callback, and adapter
payloads, including target-safe reflector examples, receipted reflection
results whose top-level source refs back the diagnosis, non-empty nested
diagnosis source refs when the optional nested field is present, proposal
reflection/result separation that preserves reflection source refs, allowed
change schema declarations,
output contexts, and payload-schema fingerprints. They are not an agent runtime,
LM prompt renderer, ACP delivery path, proposal application engine, or proof
that every optimizer/runtime producer emits these payloads.

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
  scorer assessed the true candidate output. Its execution-result verifier
  checks representative query/call/write receipt hashes against Plan IR
  preimages, including capability-authorized `case_query.load` target reads, and
  rejects same-prefix mismatches, but it is not a general ACP replay service,
  evaluator runtime target loader, or provider runtime audit log.
- `tests/plan_document.rs` also proves representative `lm_complete` lowering
  into `leaven-lm::LmRequest`, including developer/user/tool messages,
  tool-result ids, tools, model role, sampling stop sequences, provider hints,
  final-message output, JSON-schema output, and extension-content refusal. This
  is public-seam-to-neutral-vocabulary proof only; LM provider runtime and ACP
  delivery rows remain pending until reviewed as their own tranche.
- `tests/plan_document.rs` also proves representative `agent_run` and
  `sandbox_exec` lowering into `leaven-agent` and `leaven-workspace` primitives
  plus typed Plan Result receipt/value emission for those calls. Both routes
  require a live unreleased materialization-proven `workspace_handle` dependency
  before the host receives the request. Agent lowering preserves schema-valid
  `json_schema` output through the owning
  `leaven-agent::OutputContract::JsonSchema` primitive. This is public-seam
  harness proof only; provider runtime, sandbox backend execution, streaming
  delivery, and proposal parsing remain pending.
- `tests/plan_document.rs` also proves representative `workspace_materialize`
  and `workspace_release` lowering, typed `workspace_handle` value emission,
  materialize host-result checks for path-shaped workspace ids and lifetime
  substitution, provenance tracking that rejects literal forged handles and
  reuse after release, release lifecycle state, call receipts, and refusal of
  unmaterialized or host-path workspace substitutes. This is public-seam
  lifecycle proof only; concrete workspace backend execution and full
  artifact/snapshot behavior remain pending.
- `tests/call_authority.rs` proves schema-valid Call ops are checked against
  capability-granted input data classes and call-local forbidden data-class
  intersections before execution. It rejects `case.target` and other forbidden
  classes even when the Plan IR schema itself accepts the call shape. It does
  not execute LM, agent, sandbox, or human-review runtimes.
- `tests/proposal_authority.rs` proves schema-valid proposal writes are checked
  against capability-granted proposal effects, change schemas, surface
  fingerprints, and apply permission. It rejects submit-only apply attempts,
  ungranted surfaces, ungranted change schemas, and effects outside the grant.
  It does not apply proposals, mutate the graph, parse workspace diffs, or
  validate agent-session patch contents.
- `tests/plan_dialects.rs` proves pinned JSON Pointer, JSONPath, and strict
  Mustache dialects are parsed and replayed deterministically, and rejects
  unpinned path syntax, non-subset JSONPath filters/functions/scripts, non-
  strict template dialects, partials, unescaped templates, delimiter changes,
  and custom-filter syntax.
- `tests/evidence_envelope.rs` proves active-schema EvidenceEnvelope values
  preserve visibility projections, projection data classes, top-level target-
  derived data classes, and read/effect/write source receipt refs at the public-
  seam validation layer. It rejects target-derived evidence without a read
  receipt and evidence with no source receipts. It does not prove
  evaluator/evidence runtime production or public PlanResult projection.
- `tests/evaluation_job.rs` proves active-schema EvaluationJob values preserve
  evaluator/request/candidate/case/revision/deadline/capability identity for
  independent, pairwise, and listwise shapes, rejects missing deadline,
  evaluator fingerprint, capability fingerprint, unresolved case sets, and
  self-pairs, and validates `request_evaluation` Plan Result receipts against
  the job's candidate/case identity and audit hashes. Generic Plan Result
  validation rejects `request_evaluation` receipts without this job context so
  decorative request-evaluation hashes do not pass through the ordinary result
  route. It does not prove the runtime evaluator creates those jobs or emits
  evaluation request receipts unless paired with the `leaven-run` runtime
  projection tests.
- `tests/output_record.rs` proves reusable `leaven-evidence` output records
  project into the locked public-seam OutputRecord wire shape with visibility,
  data classes, non-placeholder inline output, and public blob audit metadata.
  It does not prove evaluator runtime production or pairwise/listwise
  assessment behavior.
- `tests/stage_payloads.rs` proves active-schema stage payloads have a semantic
  owner for reflector, reflection result, proposer, runner, scorer, judge,
  callback, and adapter roles. It rejects reflector `case.target` data-class
  leakage, unreceipted or diagnosis-free reflection results, dropped reflection
  source refs in proposer payloads, and change proposals without allowed change
  schema authority. It does not prove runtime stage lowering, ACP transport,
  provider calls, or proposal graph mutation.
- `tests/acp_profile.rs` proves locked Leaven ACP profile semantics for pinned
  ACP version, stdio-first transport preference, Leaven-only extension methods,
  capability-action mapping, locked Plan IR/Plan Result schema bindings,
  bounded update declarations, programmatic capability-grant permission
  decisions bound to authenticated sessions, `PlanError`/redaction denials,
  active-schema extension-result primary/receipt payloads across the full
  locked callback surface, method-specific primary value families,
  receipt-category binding, ACP-envelope JCS `result_hash` binding even for
  locked-schema primary values that cannot carry their own receipt field,
  primary receipt binding when the primary schema includes a receipt,
  and monotonic result data-class coverage. It rejects MCP/private-process substitutes,
  unpinned/latest ACP versions, non-stdio-first transport drift,
  human/always-grant permission substitutes, unbounded update declarations,
  archived/private schema bindings, bare method-specific result payloads,
  cross-method payloads, wrong receipt classes, unschematized primary/receipt
  payloads, unbound or forged result hashes including generic extension and
  receiptless workspace primaries, unbound primary receipts, and
  result data-class gaps. It does not prove ACP process startup,
  engine-client/worker-agent runtime inversion, cancellation, progress updates,
  backpressure behavior, provider calls, or worker lifecycle control.
- `tests/plan_result.rs` proves active-schema Plan Result envelopes carry typed
  success and failure values, query/call/write audit receipts, errors, charges,
  capability and policy fingerprints, receipt timing, data classes, and closed
  `PlanError` values at the public-seam validation layer. It also rejects
  same-prefix `result_hash` values that do not bind the referenced query, call,
  `submit_assessments`, or generic write result value, and paid failed calls
  whose linked charge receipts are missing, point elsewhere, or do not cover the
  failed call cost. It does not prove an engine/run producer emits those
  envelopes.
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
