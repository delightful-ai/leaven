## Boundary

This crate owns the locked V1 public seam wire contract for external-language
workers: manifest inventory, active schema/profile loading, contract
fingerprints, conformance-matrix harness data, and deferred-marker enforcement.
It also owns the wire-level capability document shape, opaque-token resolution
guardrails, and grant-envelope authorization checks needed before runtime
owners spend budgets or execute effects.

It is not a worker runtime, graph mutation layer, provider adapter, or schema
code generator. This crate may enforce seam-local monotonic data-class
projection for the wire values it builds and validates; cross-stage/runtime
data-class propagation, aggregate budget spending, and runtime behavior must
land in the owning engine, agent, workspace, LM, evaluator, or run crates and
be exercised through this seam before any conformance row can claim integrated
behavior.

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
`AcpPermissionDecision`, `AcpExtensionResultDocument`,
`AcpJsonRpcRequestDocument`, and `AcpJsonRpcResponseDocument` are advanced
public seam contracts. They prove locked Leaven ACP profile semantics,
including the exact V1 extension-method set; authenticate resolution from
opaque capability tokens through the capability registry; authenticated-session
binding for programmatic permission decisions against capability grants; typed
denial envelopes; locked Plan IR/Plan Result schema binding for profile
methods; JSON-RPC 2.0 request/response envelope binding for Leaven extension
methods with closed top-level request/response members; and schema-backed,
hash-bound extension-result envelopes at the wire-contract layer only. Generic
ACP `extension` primaries are checked against
the locked schema branch and ACP envelope fields, while concrete PlanResult
value kinds still run the full PlanResult semantic validator. LM, agent, and
sandbox ACP extension primaries additionally bind their cost object to the
carried call receipt cost, so ACP envelopes cannot shrink or omit cost
provenance while retaining a hash-bound primary. They are not an ACP process
implementation, engine-client runtime, worker-agent runtime, provider call, or
graph mutation route.

Crate-root exports for `AcpWorkerSession`, `AcpSessionLifecycle`,
`AcpSessionState`, `AcpSessionUpdate`, `AcpSessionCancellation`,
`AcpBackpressure`, `AcpProgressPriority`, and `AcpProgressDisposition` are
advanced public seam contracts for profile-derived lifecycle facts only. They
prove the engine-client/worker-agent role vocabulary, stdio-first session
model, bounded progress-update queue, receipt-bound cancellation PlanError
facts, and locked `flow_control.backpressure` strategy at the contract layer.
They are not stdio
JSON-RPC I/O, process startup, provider execution, full ACP lifecycle control,
or production worker scheduling.

Crate-root export `AcpStdioWorkerLaunch` is an advanced public seam contract
for the locked stdio ACP launch environment only. It proves the profile-owned
`LEAVEN_CAPABILITY_TOKEN`, `LEAVEN_ENDPOINT`, and
`LEAVEN_CAPABILITY_FINGERPRINT` environment bindings, engine-client/
worker-agent role facts, and bearer-token redaction from run-artifact launch
facts. It is not stdio JSON-RPC I/O, process spawning, provider execution,
session supervision, or a full ACP transport implementation.

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
output, and JSON-schema output. `PlanLmCompleteOutcome::from_lm_response`
projects provider-neutral `leaven_lm::LmResponse` plus metered `leaven-kernel`
cost into the Plan Result outcome shape without requiring hosts to hand-write
LM response JSON. JSON-schema LM outputs must return a parsed Plan Result
payload, and successful call-result validation requires an `lm_complete` result
to be an `lm_response` value carrying the matching call receipt, matching
value/receipt cost, and an assistant-authored text final response. Result-side
tool metadata, tool results, extension content, and oversized `final_message`
text are rejected by the seam instead of being accepted as generic schema-valid
`LmMessage` values. Request-side extension/multimodal content is rejected
rather than silently downgraded to text. It still requires a concrete model
before provider execution and is not provider runtime execution, streaming, ACP
delivery, or full `ps1.lm.contract` closeout. The `PlanAgentRunRequest` and
`PlanSandboxExecRequest` routes lower schema-valid `agent_run` and
`sandbox_exec` calls into provider-neutral `leaven-agent::AgentRunRequest` and
backend-neutral `leaven-workspace::Command` primitives, and the representative
harness can emit typed `agent_session` and `sandbox_exec` Plan Result values
with call receipts. `PlanAgentRunOutcome::from_agent_session_with_command_output_refs` projects
provider-neutral `leaven_agent::AgentSession` plus metered `leaven-kernel` cost
into the Plan Result outcome shape, while requiring the host to provide the
transcript blob ref that made the transcript durable and stdout/stderr/file
blob refs for every observed command output. Those command refs are verified
against captured `leaven-workspace::CommandOutput` bytes before they can be
emitted, so unbound agent stdout cannot masquerade as proposal evidence.
JSON-schema agent outputs must return a parsed Plan Result payload, and
successful call-result validation requires `agent_run` and `sandbox_exec`
result values to carry the matching call receipt and expected value kind.
`agent_run` values must also carry a transcript blob ref, non-empty command
records with argv plus finite V1 command status facts, stdout/stderr blob refs,
safe relative output-file blob refs where captured, command refs bound to the
enclosing session receipt, and cost matched by the call receipt. `sandbox_exec` with
`stream_policy: blob_refs_only` must return stdout/stderr blob refs; completed
sandbox results must carry
`exit_code`, must carry cost matched by the call receipt, and may carry captured
output-file blob refs only at safe relative workspace paths after the host binds
declared bytes and SHA-256 to file bytes captured on the provider-neutral
`leaven-workspace::CommandOutput`. Agent transcript/command
blob refs and sandbox stdout/stderr/file blob refs are monotonically projected
into the top-level Plan Result value `data_classes` by the seam-owned outcome
builders, and forged results that drop those nested classes are rejected by
Plan Result validation. Both `agent_run` and `sandbox_exec` require a live
unreleased,
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
forgeries and host filesystem paths as workspace handles. WorkspaceRef object
form is not collapsed to a bare id: when a caller supplies `run` or
`snapshot_fingerprint`, those fields must match the materialized handle before
release, agent, sandbox, or workspace query execution can proceed. Result replay
validation also rechecks lifecycle state: materialize results must explicitly
bind an unreleased handle with the requested lifetime, and release results must
explicitly bind the same live handle as released.
`PlanWorkspaceQueryRequest` routes representative schema-valid
`workspace_query` reads through typed host requests, requires a live
materialization-proven `workspace_handle` dependency before the host can read,
rejects absolute/traversal-shaped workspace paths at the seam, and emits typed
`workspace_file`, `workspace_listing`, `workspace_snapshot`, or `workspace_diff`
values with query receipts. `stat` projects as a `workspace_listing`, `digest`
as a `workspace_snapshot`, and `git_log` as a `workspace_diff` because the
locked result schema has those workspace-read value families rather than
separate stat/digest/log value kinds. Within that broad family mapping, `stat`
still binds the result to the requested path and `digest` still binds the
result to the requested algorithm and workspace id; the locked result schema
does not carry the digest request path, so digest path-level backend truth
remains pending. `read_file` binds the returned file value to the requested path
and requires content or a blob ref. `list` results must stay under the requested
path, `snapshot` results must bind the requested workspace and carry a digest,
`git_log`/`git_diff`/`git_status` results must carry text or a blob ref inside
the locked `workspace_diff` value family, and `capture_artifacts` requests must
name at least one safe relative path with results corresponding to requested
paths. `git_log` remains a broad `workspace_diff`-family projection, not a
parsed commit-log surface.
`PlanWorkspaceQueryRequest::execute_on_workspace_view` can execute finite
read/list/stat/sha256-digest/blake3-digest/snapshot/requested-path
capture-artifact listing reads through `leaven-workspace::WorkspaceView`, giving
hosts an owned substrate path that is not hand-written result JSON. It enforces
`read_file.max_bytes`, `list.max_entries`, `list.recursive`, and
`capture_artifacts.max_bytes` within the limits of the locked result shape.
`capture_artifacts` still projects requested-path listing evidence, not a rich
artifact bundle, because the locked result schema exposes the operation through
the `workspace_listing` value family. Git-specific queries remain host-owned
because the V1 workspace substrate does not expose Git preimage fields such as
`against`, `porcelain`, or parsed log entries. This is partial finite workspace
substrate proof, not full concrete Git/artifact/snapshot backend closeout, so it
is not full workspace row closeout. The
`PublicSeamPackage::execute_plan_document_with_capability` route
checks Plan call/write authority against the supplied capability before host
effects can run, including workspace lifecycle calls and `emit_run_event`
writes, and requires capability-authorized evaluator request scope before a
`case_query.load` host read can run. The

Capability-authorized plan execution is a dynamic data-class gate as well as a
static Plan IR gate. Before any capability-scoped call reaches the host, the
seam collects `data_classes` from resolved dependency values, requires the call
to declare those classes in `input_classes`, and authorizes the effective union
against the capability grant. A call may not drop `case.target` or another
forbidden dependency class by relabeling only its own declared inputs. The
collector is limited to recognized seam wire metadata carriers and nested
blob/trace/reference fields; arbitrary application JSON fields named
`data_classes`, even inside domain records with their own `kind`, are domain
payload rather than authorization metadata unless the kind is part of the
locked public-seam value vocabulary.

The
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
JSON-schema LM and agent output contracts are executable harness contracts:
inline schemas are compiled before host effects, and returned or replayed
`parsed` payloads must validate against the requested schema. The inline schema
must match `schema_fingerprint`, so capability authority cannot be checked
against one schema while execution validates a weaker inline schema. This is
still not provider/runtime proof for structured output enforcement outside the
public-seam harness.
The proposal authority
route checks `submit_proposal_batch` and `apply_proposal_batch` writes against
capability-granted effects, surfaces, schemas, and apply permission.
`change_from_agent_session` effects must cite the same agent session receipt in
the effect and proposal `read_receipts`, so an agent-shaped change cannot pass
as a receipted proposal with only decorative stdout or omitted session
provenance. This is still a Plan IR authority check, not proof that the agent
runtime produced the session. These checks are not ACP delivery, provider
runtime execution, general cache backend behavior, graph mutation authority,
full Plan IR coverage, evaluator runtime production, or engine RunGraph
revision reads.
The sandbox execution route treats completed stdout/stderr blob refs as audit
facts, not optional decoration: live host outcomes, replayed Plan Results, and
ACP extension results must preserve them. This is still public-seam harness
validation, not a claim that a production sandbox runtime or streaming
transport has shipped.

Captured sandbox output-file refs are byte-bound through
`leaven-workspace::CommandOutput`: the public outcome constructor requires every
captured file to have a matching blob ref, rejects extra file refs, rejects
truncated captures, and checks SHA-256 and byte counts before the value can be
recorded. Replayed foreign Plan Results can still validate only the locked wire
facts they carry; production file capture remains a sandbox backend concern.

Crate-root export `PinnedDialectEvaluator` is an advanced public seam contract.
It proves deterministic parsing and replay for the V1 pinned wire
mini-languages: RFC 6901 JSON Pointer, the Leaven RFC 9535 JSONPath subset, and
`leaven.mustache.strict.v1`. It is not a full Plan IR executor, graph query
engine, template-extension host, or authorization layer.

Crate-root exports for `PlanResultDocument` and `Replayability` are advanced
public seam contracts. They prove active-schema plan-result envelope validation,
typed value/receipt/error/charge classification, operation receipt timing,
closed `PlanError` shape, replayability roll-up, and monotonic value
data-class coverage for nested score outputs, embedded evidence, top-level
trace refs, blob refs, and workspace-listing entries at the wire-envelope layer
only. `graph_set` assessment summaries must carry a `Score.output` with
candidate output/artifact data classes plus an `EvidenceEnvelope` whose source
receipts are present in the Plan Result receipt set; a graph row shaped like an
assessment without output/evidence truth is rejected. These exports are not
plan-run production, evaluator execution, runtime receipt production, graph
mutation, or cache replay behavior.

Crate-root export `EvidenceEnvelopeDocument` is an advanced public seam
contract. It proves active-schema evidence-envelope visibility, data-class, and
source-receipt preservation plus target-derived data-class coverage at the
wire-envelope layer only. Envelope-level and public trace-ref data classes are
part of that coverage, and target-derived evidence must also carry a read
receipt so target-derived facts cannot pass as unreceipted policy metadata.
Evidence that carries `case.target` data classes must declare
`target_derived=true`, so a false target-derived flag cannot hide target
material in public, private, trace, or top-level data-class projections.
When non-target evidence declares top-level `data_classes`, that declaration
must still cover public, private, and trace projection classes, so declared
envelope classes cannot shrink the visible propagation set. Private
`payload_ref` blob data classes must be covered by the private projection data
classes before the envelope can pass. When those envelope source receipts are
embedded in a Plan Result as object-form receipt refs, their fingerprints must
match the actual carried receipt objects instead of stale or decorative receipt
metadata. This is not evaluator evidence production, redaction execution,
receipt persistence, or data-class propagation through runtime stages.

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
data-classes, and blob audit metadata. Audited evidence blob records can supply
that metadata directly; unaudited `OutputRecord::blob(...)` values still need an
explicit `PublicBlobRef` at projection time. They are not proof that evaluator
runtimes produce those records for every assessment shape.

Crate-root exports for `StagePayloadDocument`, `ReflectProposeHandoffDocument`,
`StagePayloadRole`, and `StageProposalEffect` are advanced public seam
contracts. They prove active-schema stage-payload validation for role-specific
reflector, reflection-result, proposer, runner, scorer, judge, callback, and
adapter payloads, including non-empty target-safe reflector examples whose
source refs and nested score-output data classes are carried by the request,
reflector source refs that cannot hide `case.target` markers, receipted
reflection results whose top-level source refs and required nested diagnosis
source refs back the diagnosis, proposal/reflection separation that preserves
reflection source refs, active reflect-then-propose handoff binding with
distinct reflector/proposer stage call ids, exact `ReflectionResult`
consumption, stage receipts that fingerprint the produced reflection result and
bind proposer consumption back to the reflector receipt, shared
run/revision/parent/surface/capability/query-policy facts, allowed change
schema declarations, target-aware scorer context binding to the scored case,
score/judge output contexts that at least declare assessed candidate or
artifact output data classes, and payload-schema fingerprints.
That data-class declaration is necessary seam metadata; it is not independent
proof that arbitrary stage JSON is the actual candidate/artifact output being
assessed. They are not an agent runtime, LM prompt renderer, ACP delivery path,
proposal application engine, or proof that every optimizer/runtime producer
emits these payloads.

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
  `Score.output` checks require candidate/artifact data classes, a matching
  `evidence.public.summary` projection, an embedded evidence envelope that
  passes the same semantic evidence checks as standalone evidence, and either a
  candidate-bound value matching the assessment's `candidate` / `candidates`
  field or an explicit blob/trace output projection. The embedded evidence
  source receipts are the receipt carrier for this locked Plan IR shape; the
  optional assessment-level `read_receipts` and `effect_receipts` fields are
  not required duplicate declarations. This rejects unbound, summary-only,
  mismatched-candidate, unreceipted-evidence, and wrong-receipt-family schema
  dummies, but it is
  still self-declared public-seam document validation, not proof that the value
  is the actual candidate/artifact output assessed, and not provider or ACP
  runtime proof. Its execution-result verifier
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
  `leaven-agent::OutputContract::JsonSchema` primitive, and agent session audit
  validation rejects missing command receipts, invalid argv, arbitrary command
  status strings, and command programs outside declared `allowed_commands`.
  This is public-seam harness proof only; provider runtime, sandbox backend
  execution, streaming delivery, and proposal parsing remain pending.
- `tests/plan_document.rs` also proves representative `workspace_materialize`
  and `workspace_release` lowering, typed `workspace_handle` value emission,
  materialize host-result checks for path-shaped workspace ids and lifetime
  substitution, provenance tracking that rejects literal forged handles and
  reuse after release, release lifecycle state, call receipts, and refusal of
  unmaterialized or host-path workspace substitutes. This is public-seam
  lifecycle proof only; concrete workspace backend execution and full
  artifact/snapshot behavior remain pending.
- `tests/workspace_query_contract.rs` proves finite workspace-query reads can
  execute through the public seam into `leaven-workspace::WorkspaceView` for
  read_file, list, stat, sha256/blake3 digest, snapshot, and requested-path
  capture-artifact listing. It also proves read/capture byte bounds, list entry
  bounds, and refusal of git_log/git_diff/git_status by the generic helper
  instead of faking Git preimage truth. This is finite substrate proof only;
  concrete Git workspace behavior and richer artifact bundle capture remain
  host/back-end owned and pending.
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
  receipt, evidence with no source receipts, and envelopes that carry
  `case.target` data classes while claiming `target_derived=false`, as well as
  private payload-ref data classes not covered by the private projection,
  top-level target-derived class gaps, and declared non-target top-level class
  sets that drop public/private/trace projection classes. It does not prove
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
  leakage, target markers hidden in reflector source refs, empty reflector
  example sets, reflector examples whose source refs are not carried by the
  request, reflector example data-class gaps for nested score outputs,
  unreceipted or diagnosis-free reflection results, diagnosis entries without
  carried source refs, dropped reflection source refs in proposer payloads,
  object-form candidate refs whose optional `run` is substituted while keeping
  the same candidate id,
  one-stage reflect/propose handoff substitutions, stale or mismatched embedded
  reflection results, missing or mismatched stage receipt bindings, mismatched
  run/capability facts, and change proposals without allowed change schema
  authority. It also rejects public-only scorer and judge outputs that satisfy
  schema shape but do not declare candidate/artifact output provenance, plus
  scorer and judge outputs whose nested blob or trace data classes are not
  covered by the enclosing output record. It does not prove runtime stage
  lowering, ACP transport, provider calls, proposal graph mutation, or
  independent output-identity truth for arbitrary stage JSON.
- `tests/acp_profile.rs` proves locked Leaven ACP profile semantics for pinned
  ACP version, stdio-first transport preference, Leaven-only extension methods,
  capability-action mapping, locked Plan IR/Plan Result schema bindings,
  bounded update declarations, profile-derived engine-client/worker-agent
  session facts, bounded progress-update queue behavior, lifecycle cancellation
  state, programmatic capability-grant permission decisions bound to authenticated
  sessions, `PlanError`/redaction denials,
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
  stdio JSON-RPC I/O, provider calls, or full worker lifecycle control.
- `tests/plan_result.rs` proves active-schema Plan Result envelopes carry typed
  success and failure values, query/call/write audit receipts, errors, charges,
  capability and policy fingerprints, receipt timing, data classes, and closed
  `PlanError` values at the public-seam validation layer. It also proves
  `graph_set` assessment summaries cannot omit `Score.output` or carry
  unreceipted/semantically invalid evidence envelopes, and rejects
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
  source receipt refs that are missing, categorized as the wrong
  query/call/write receipt kind, or carry stale object-form receipt
  fingerprints. It does not prove evaluator evidence production, runtime
  receipt persistence, or full data-class propagation across query/call/write
  execution.
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
