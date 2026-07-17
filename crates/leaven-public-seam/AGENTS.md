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
  budget truth. Capability document validation also enforces locked
  subject/grant invariants that JSON Schema cannot encode: runner/reflector
  stage-call subjects cannot receive target-bearing grants, and
  `evaluation_stage_call` assessment-submit grants must stay within the
  subject evaluation request.
- Grant-envelope authorization for action, resource selectors, case fields,
  partitions, schemas, surface fingerprints, data classes, and per-grant limits.
- RFC 8785 JCS plus SHA-256 schema fingerprint values such as
  `fp_schema_sha256_*`.
- Conformance-matrix row parsing, uniqueness checks, spec-reference checks,
  notes-denominator parsing, and row evidence/status helpers.
- V1 hard-cutover markers: MCP is not V1, `watch.v1` runtime behavior is
  deferred, and `worker_protocol.v1` is deprecated in favor of the Leaven
  worker profile. Legacy `acp` identifiers in this crate refer to that
  Leaven-owned worker profile unless a doc explicitly says upstream Agent
  Client Protocol.

## Route Away

- Cold optimizer vocabulary stays in `leaven-core`.
- Mechanical BLAKE3 behavior fingerprints stay in `leaven-kernel`; this crate's
  schema fingerprints are public-seam wire identifiers, not cache behavior
  fingerprints.
- Graph mutation remains private to `leaven-engine` through `RunContext`.
- Worker process/session behavior belongs in `leaven-acp`, not in this crate. The
  current route note is
  `docs/plans/2026-05-24-public-seam-v1-acp-transport-route.md`: keep the
  Leaven `leaven/*` method/result contract here, prove transport behavior with
  black-box subprocess tests before promoting worker-transport rows. Do not add
  upstream ACP SDK dependencies or claim upstream ACP conformance without a new
  agent-provider interoperability design slice.
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
document authority resolution plus mint-time subject/grant semantic checks
only; they are not ACP authentication, runtime evaluator closeout, or
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
legacy-worker exclusion only; they are not a worker process implementation,
session lifecycle, authentication handshake, permission loop, or worker runtime.

Crate-root exports for `AcpProfileDocument`, `AcpExtensionMethod`,
`AcpAuthenticateRequest`, `AcpAuthenticatedSession`, `AcpPermissionRequest`,
`AcpPermissionDecision`, `AcpExtensionResultDocument`,
`AcpJsonRpcRequestDocument`, and `AcpJsonRpcResponseDocument` are advanced
public seam contracts. They are legacy-named types for the locked Leaven worker
profile; they do not prove upstream Agent Client Protocol conformance. They
prove locked Leaven worker profile semantics, including the exact V1 method set;
authenticate resolution from
opaque capability tokens through the capability registry; authenticated-session
binding for programmatic permission decisions against capability grants; typed
denial envelopes; per-method schema binding for profile methods (the 25
worker->host effect callbacks bind locked Plan IR params plus Plan Result
results, while the one host->worker `leaven/stage.run` dispatch method binds the
dedicated `leaven.stage_run.v1` request/result schema); JSON-RPC 2.0
request/response envelope binding for Leaven extension
methods with closed top-level request/response members; a parallel JSON-RPC
envelope binding for the `leaven/stage.run` dispatch
(`validate_acp_stage_run_request_document` /
`validate_acp_stage_run_response_document`, exported as
`AcpStageRunRequestDocument` / `AcpStageRunResponseDocument`) that gates the
method through the profile, carries the stage-run schema instead of Plan IR, and
binds the response id to the dispatched request id; and schema-backed,
hash-bound extension-result envelopes at the wire-contract layer only. Generic
`extension` primaries are checked against
the locked schema branch and worker-profile envelope fields, while concrete PlanResult
value kinds still run the full PlanResult semantic validator. LM, agent, and
sandbox worker-profile extension primaries additionally bind their cost object to the
carried call receipt cost, so worker-profile envelopes cannot shrink or omit cost
provenance while retaining a hash-bound primary. They are not a worker process
implementation, upstream ACP runtime, provider call, or graph mutation route.

Crate-root exports for `AcpWorkerSession`, `AcpSessionLifecycle`,
`AcpSessionState`, `AcpSessionUpdate`, `AcpSessionCancellation`,
`AcpBackpressure`, `AcpProgressPriority`, and `AcpProgressDisposition` are
advanced public seam contracts for profile-derived lifecycle facts only. They
prove the engine-client/worker-agent role vocabulary, stdio-first session
model, bounded progress-update queue, receipt-bound cancellation PlanError
facts, and locked `flow_control.backpressure` strategy at the contract layer.
They are not stdio
JSON-RPC I/O, process startup, provider execution, upstream ACP lifecycle control,
or production worker scheduling.

Crate-root export `AcpStdioWorkerLaunch` is an advanced public seam contract
for the locked stdio Leaven worker launch environment only. It proves the profile-owned
`LEAVEN_CAPABILITY_TOKEN`, `LEAVEN_ENDPOINT`, and
`LEAVEN_CAPABILITY_FINGERPRINT` environment bindings, engine-client/
worker-agent role facts, and bearer-token redaction from run-artifact launch
facts. It is not stdio JSON-RPC I/O, process spawning, provider execution,
session supervision, or upstream ACP transport implementation.

Codex app-server is a provider runtime leaf, not worker transport proof by
itself. A Codex-backed path can count as public-seam worker evidence only when
it crosses a real Leaven worker process/session boundary and returns locked
Leaven extension results through this crate's validators.

Crate-root exports for `PlanDocument`, `PlanOperationKind`,
`PlanExecutionContext`, `PlanExecutionHost`, `PlanExecutionReport`,
`PlanGraphReadScope`, `PlanGraphQueryRequest`, `PlanGraphQueryOutcome`,
`PlanCaseQueryRequest`, `PlanCaseQueryOutcome`, `PlanLmCompleteRequest`,
`PlanLmCompleteOutcome`, `PlanAgentRunRequest`, `PlanAgentRunOutcome`,
`PlanSandboxExecRequest`, `PlanSandboxExecOutcome`,
`PlanWorkspaceMaterializeRequest`, `PlanWorkspaceMaterializeOutcome`,
`PlanWorkspaceReleaseRequest`, `PlanWorkspaceReleaseOutcome`,
`PlanEmitRunEventRequest`, `PlanEmitRunEventOutcome`, `CallAuthorityReport`,
`CallAuthorityError`, `CallAuthorityDenial`, `CallAuthorityDenialKind`, and
`ProposalAuthorityReport` are advanced public seam contracts. They prove
active-schema Plan IR document validation and Let/Call/Write family
classification plus representative lowering/execution of literal Let,
`graph_query` reads, `case_query.load` reads, `lm_complete` Call, and
`emit_run_event` Write into a validated Plan Result with query/call/write
receipts. The execution harness lowers schema-valid `lm_complete` calls into
provider-neutral `leaven-lm` vocabulary before invoking the host, so hosts
receive a `PlanLmCompleteRequest` exposing the lowered `LmRequest` rather than
raw call JSON. That route preserves developer/user/tool messages,
tool-result ids, tool definitions, model role, sampling stop sequences,
provider hints, final-message output, and JSON-schema output.
`PlanLmCompleteOutcome::from_lm_response`
projects provider-neutral `leaven_lm::LmResponse` plus metered `leaven-kernel`
cost into the Plan Result outcome shape; successful LM outcomes are not
publicly constructible from hand-written response JSON or ad hoc cost fields.
`PlanLmCompleteRequest::execute_with_lm` is the seam-owned adapter from a
lowered Plan IR request into an `impl leaven_lm::Lm`: hosts provide the LM
capability, while this crate preserves the lowered request, runtime
fingerprint, metered cost, and JSON-schema parsed payload behavior. It is not a
provider-specific client, live network execution proof, ACP delivery proof, or
streaming runtime.
JSON-schema LM outputs must return a parsed Plan Result
payload, and successful call-result validation requires an `lm_complete` result
to be an `lm_response` value carrying the matching call receipt, matching
value/receipt cost, and an assistant-authored text final response. Result-side
tool metadata, tool results, extension content, and oversized `final_message`
text are rejected by the seam instead of being accepted as generic schema-valid
`LmMessage` values. Request-side extension/multimodal content is rejected
rather than silently downgraded to text, and streaming-shaped LM requests are
rejected before host execution. It still requires a concrete model before
provider execution and is not live provider execution, ACP delivery, or runtime
streaming delivery. The `PlanAgentRunRequest` and `PlanSandboxExecRequest`
routes likewise expose already-lowered provider-neutral
`leaven-agent::AgentRunRequest` and backend-neutral `leaven-workspace::Command`
primitives to hosts instead of raw `agent_run`/`sandbox_exec` call JSON. These
calls are only executable through
`PublicSeamPackage::execute_plan_document_with_capability`; the no-capability
Plan execution route rejects them before workspace materialization, agent host
calls, or sandbox host calls. The representative harness can emit typed
`agent_session` and `sandbox_exec` Plan Result values with call receipts. Agent
request lowering preserves the Plan IR runtime selector and optional runtime
fingerprint; when a fingerprint is declared, the Plan Result call receipt must
come back from that fingerprint.
`PlanAgentRunOutcome::from_agent_session_with_command_output_refs` projects
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
rejects absolute/traversal-shaped workspace paths at the seam, and, when a
capability document is supplied, authorizes `workspace.read` by workspace id,
workspace operation, and data class before any host read can run. It emits typed
`workspace_file`, `workspace_listing`, `workspace_snapshot`, or `workspace_diff`
values with query receipts. `stat` projects as a `workspace_listing`, `digest`
as a `workspace_snapshot`, and Git queries as `workspace_diff` because the
locked result schema has those workspace-read value families rather than
separate stat/digest/log value kinds. Within that broad family mapping, `stat`
binds the result to the requested path; `digest` binds the result to the
requested algorithm, workspace id, and requested path through a
`source_refs.external` value; `git_log`, `git_diff`, and `git_status` bind their
request-specific controls through `source_refs.external` values while still
requiring text or a blob ref inside the locked `workspace_diff` family.
`read_file` binds the returned file value to the requested path and requires
content or a blob ref. `list` results must stay under the requested path,
`snapshot` results must bind the requested workspace and carry a digest, and
`capture_artifacts` requests must name at least one safe relative path with
results corresponding to requested paths. Git queries remain broad
`workspace_diff`-family projections, not parsed Git surfaces.
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
forbidden dependency class by relabeling only its own declared inputs. Literal
`Expr.data_classes` travel as separate binding metadata so the host-visible
dependency value is not rewritten just to satisfy authority checks. The
collector is limited to recognized seam wire metadata carriers, output-record
shapes, workspace listing entries, graph rows, nested blob refs, command refs,
trace refs, evidence projections, and stage/result output fields. Arbitrary
application JSON fields named `data_classes`, even inside domain records with
their own `kind`, are domain payload rather than authorization metadata unless
the object matches the locked public-seam value or output-record vocabulary.
This is call-authority evidence before host call execution; write-path
dependency propagation is proven only for the representative `emit_run_event`
write harness, where dependency binding classes are exposed to the host request
without rewriting dependency JSON values. Literal `Expr.data_classes` are bound
into representative call/write request hashes and reconstructed during receipt
validation. Public call-authority validation reports typed
`CallAuthorityDenial` data-class refusals with redaction class names for
capability-forbidden input classes, call-local `forbidden_input_classes`, and
reflector LM target egress. Reflector LM target-egress checks cover both
capability subject role and call-local `model_role: "reflector"` so a broad
grant on a non-reflector subject cannot hide target egress behind the grant.
Representative capability-scoped plan execution preserves those data-class
redaction facts in `PublicSeamError::CallAuthorityDenied` when dependency-side
classes are denied before any host call runs. This still is not full
engine/evidence-layer propagation, Plan Result `Redaction` wire-object
reporting, ACP/provider runtime behavior, or proof for every production
query/call/write route.

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
runtime produced the session. These checks are not worker transport delivery, provider
runtime execution, general cache backend behavior, graph mutation authority,
full Plan IR coverage, evaluator runtime production, or engine RunGraph
revision reads.
The sandbox execution route treats completed stdout/stderr blob refs as audit
facts, not optional decoration: live host outcomes, replayed Plan Results, and
worker-profile extension results must preserve them. This is still public-seam harness
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
`ReflectProposeSubmissionDocument`, `StagePayloadRole`, and
`StageProposalEffect` are advanced public seam contracts. They prove
active-schema stage-payload validation for role-specific reflector,
reflection-result, proposer, runner, scorer, judge, callback, and adapter
payloads, including non-empty target-safe reflector examples whose source refs
and nested score-output data classes are carried by the request, reflector
source refs that cannot hide `case.target` markers, receipted reflection results
whose top-level source refs and required nested diagnosis source refs back the
diagnosis, proposal/reflection separation that preserves reflection source
refs, active reflect-then-propose handoff binding with distinct
reflector/proposer stage call ids, exact `ReflectionResult` consumption, stage
receipts that fingerprint the produced reflection result and bind proposer
consumption back to the reflector receipt, proposal submissions that cite that
proposer receipt in a top-level literal array and preserve reflection read
receipts, reflected parent causal input, allowed effects, allowed change
schemas, and change target/surface facts, shared
run/revision/parent/surface/capability/query-policy facts, allowed change schema
declarations, target-aware scorer context binding to the scored case,
score/judge output contexts that at least declare assessed candidate or artifact
output data classes, and payload-schema fingerprints.
That data-class declaration is necessary seam metadata; it is not independent
proof that arbitrary stage JSON is the actual candidate/artifact output being
assessed. The proposal-submission check is an exact cited-handoff validation,
not a typed schema-level provenance edge or capability authorization proof.
They are not an agent runtime, LM prompt renderer, ACP delivery path, proposal
application engine, RunContext graph mutation proof, receipt-store persistence
proof, or proof that every optimizer/runtime producer emits these payloads.

Crate-root exports for `StageRunRequestDocument`, `StageRunResultDocument`,
`StageRunKind`, `StageScoreFact`, and `StageRewardFact` are advanced public seam
contracts for the host->worker stage dispatch leg. They prove active-schema
`leaven.stage_run.v1` validation for the one generic `leaven/stage.run` method: a
dispatch request carries a stage kind plus a role-scoped stage payload, and the
embedded payload is re-validated through the same role-scoped stage-payload
semantic checks so case-target material cannot ride a runner-stage dispatch past
the runner guard; a dispatch result returns the stage's typed `OutputRecord` (a
text output). V1 dispatches the target-free runner stage, the scorer stage (a
`ScoreContext` payload whose result must carry a typed reward-vector
`StageScoreFact` with finite per-reward values), and the proposer stage; the
score-presence law refuses scorer results without a reward vector and refuses
runner/proposer results that smuggle one. This is intentionally separate from
the Plan IR effect callbacks: `leaven/stage.run` binds the stage-run schema in
both directions and does not bind Plan IR or Plan Result. They are not a worker
process implementation, transport dispatch loop, stage execution runtime, or
graph mutation route; scorer-stage worker serving lands in a later slice. The
locked capability invariant that runner/reflector stage-call subjects cannot
receive target-bearing grants does not extend to scorer subjects: scoring reads
the case target through capability-gated case access.

Crate-root exports for `OptimizeRunRequestDocument`, `OptimizeRunResultDocument`,
`OptimizeObjective`, `OptimizeReflection`, `OptimizeSplit`, `OptimizerConfig`,
`OptimizeCase`, `ArtifactRecord`, `ArtifactPayload`, `SkillFile`,
`CandidateEntry`, and `OptimizeRunReference` (plus the `PROMPT_ARTIFACT_TYPE` and
`AGENT_KIT_ARTIFACT_TYPE` constants)
are advanced public seam contracts for the client->host optimization-dispatch
leg. They prove active-schema `leaven.optimize_run.v1` validation for the one
`leaven/optimize.run` method: a request carries a seed artifact record
(the same `{artifact_type, artifact_schema, artifact}` triple a proposal
`create` effect carries) whose payload parses into a typed `ArtifactPayload`
discriminated by `artifact_type` (a `prompt` template, or an `agent_kit`
projection of a Git-backed AgentKit revision: a `system_prompt` slot plus
path-validated, path-unique `SkillFile` records mounted under the Codex
`.agents/skills` mount). The `agent_kit` record is a projection of a Git-backed
artifact, not the artifact itself: the host owns run-scoped repository
construction and child-revision readback, and the seam enforces the AgentKit
path law (no absolute, parent-traversal, current-directory, empty-component,
backslash, NUL, or duplicate skill paths) that JSON Schema cannot cleanly
encode. A request also carries
a non-empty target-bearing case manifest, optimizer
config (a finite `max_metric_calls`, optional population/minibatch sizes, and a
typed objective parsing all four `instance`/`objective`/`hybrid`/`cartesian`
variants where `hybrid`/`cartesian` are validate-only at the service layer), and
a reflection config (`lm` with a model name, or `agentic`). Targets are allowed
on the case manifest precisely because the document goes to the host, which owns
target custody; runner stage payloads still never carry targets. A result
carries the optimized projection: best candidate, frontier, iteration and
metric-call counts, aggregate cost, the durable run/revision reference, and
applied proposal-batch receipts. The semantic laws require finite candidate
scores and that `best` appears in `frontier` (matched by candidate id), so the
projection cannot claim a best candidate the frontier never admitted.
`leaven/optimize.run` is a third method direction: it is not a worker->host
callback or the host->worker stage dispatch, so the worker profile does not
advertise it (`LockedMethod::is_worker_profile_method` is false for it and
`LockedMethod::WORKER_PROFILE` excludes it). These are not an optimizer runtime,
GEPA host, run/checkpoint store, graph mutation route, or worker transport;
configured service execution of `leaven/optimize.run` lands with the GEPA host
slice of the active production goal.

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
  is the V1 public-seam LM contract proof cited by the matrix. Concrete LM
  provider calls and provider-specific streaming/multimodal behavior remain
  outside this crate and outside the V1 default public route.
- `tests/plan_document.rs` also proves representative `agent_run` and
  `sandbox_exec` lowering into `leaven-agent` and `leaven-workspace` primitives
  plus typed Plan Result receipt/value emission for those calls. Both routes
  require a live unreleased materialization-proven `workspace_handle` dependency
  before the host receives the request. Agent lowering preserves schema-valid
  `json_schema` output through the owning
  `leaven-agent::OutputContract::JsonSchema` primitive, and agent session audit
  validation rejects missing command receipts, invalid argv, arbitrary command
  status strings, and command programs outside declared `allowed_commands`.
  The public execution route also rejects `agent_run`/`sandbox_exec` without a
  capability document, while the capability route checks workspace ids, input
  classes, schema/surface grants, execution policy, subprocess permissions, and
  command grants before host execution.
  This is the V1 public-seam agent/sandbox contract proof cited by the matrix.
  Concrete provider adapters, sandbox backends, and proposal parsers remain
  separate owner routes rather than hidden requirements for this crate.
- `tests/plan_document.rs` also proves representative `workspace_materialize`
  and `workspace_release` lowering, typed `workspace_handle` value emission,
  materialize host-result checks for path-shaped workspace ids and lifetime
  substitution, provenance tracking that rejects literal forged handles and
  reuse after release, release lifecycle state, call receipts, and refusal of
  unmaterialized or host-path workspace substitutes. This is the V1 public-seam
  workspace lifecycle proof cited by the matrix. Concrete workspace backend
  execution and richer artifact/snapshot product behavior remain separate
  workspace-backend owner routes.
- `tests/workspace_query_contract.rs` proves finite workspace-query reads can
  execute through the public seam into `leaven-workspace::WorkspaceView` for
  read_file, list, stat, sha256/blake3 digest, snapshot, and requested-path
  capture-artifact listing. It also proves read/capture byte bounds, list entry
  bounds, and refusal of git_log/git_diff/git_status by the generic helper
  instead of faking Git preimage truth. This is the finite V1 public-seam query
  proof. Concrete Git workspace behavior and richer artifact bundle capture
  remain host/back-end owned route-away work.
- `tests/call_authority.rs` proves schema-valid Call ops are checked against
  capability-granted input data classes and call-local forbidden data-class
  intersections before execution. It rejects `case.target` and other forbidden
  classes even when the Plan IR schema itself accepts the call shape, and it
  reports typed call-authority redaction facts. It does not produce Plan Result
  `Redaction` wire objects or execute LM, agent, sandbox, or evaluator
  runtimes.
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
  lowering, worker transport, provider calls, proposal graph mutation, or
  independent output-identity truth for arbitrary stage JSON.
- `tests/stage_run.rs` proves the host->worker `leaven/stage.run` dispatch wire:
  schema-valid runner, scorer, and proposer dispatch requests (stage kind plus
  role-scoped payload) and their text-output dispatch results validate through
  `validate_stage_run_*`. A scorer result carries a typed reward-vector
  `StageScoreFact`; the score-presence law refuses a scorer result without a
  reward vector and refuses runner/proposer results that carry one, and refuses
  wrong-typed or non-finite reward/score numbers. A request carrying
  `case.target` material or a payload role that does not match the dispatched
  stage kind, a non-text result output, and a Plan Result envelope smuggled as a
  stage-run result are all rejected. It does not prove worker transport dispatch
  delivery, stage execution, or reflector/judge stage kinds (deferred to later
  slices).
- `tests/optimize_run.rs` proves the client->host `leaven/optimize.run` dispatch
  wire: a schema-valid request (seed artifact record, non-empty target-bearing
  case manifest with a null-target case, optimizer config with all four typed
  objectives, and `lm`/`agentic` reflection) and a schema-valid result whose best
  candidate appears in the frontier validate through
  `validate_optimize_run_*`. Both artifact projections parse into typed payloads:
  a `prompt` template, and an `agent_kit` projection whose `system_prompt` and
  path-validated `SkillFile` records round-trip through the request and result
  accessors. It rejects an empty case manifest, a missing message
  discriminator, an objective outside the locked enum, zero `max_metric_calls`, a
  missing case `target` field, a best candidate not present in the frontier, an
  empty frontier, non-finite/non-numeric scores, malformed `applied_proposals`
  receipts, an agent_kit projection missing `system_prompt`, and a skill path
  that is absolute, carries parent traversal, or duplicates another skill path.
  It does not prove an optimizer runtime, GEPA host, run/checkpoint
  readback, or worker transport; configured service execution lands with the GEPA
  host slice of the active production goal.
- `tests/acp_profile.rs` proves locked Leaven worker profile semantics for pinned
  worker-profile version, stdio-first transport preference, Leaven-only seam methods,
  capability-action mapping, locked Plan IR/Plan Result schema bindings,
  bounded update declarations, profile-derived engine-client/worker-agent
  session facts, bounded progress-update queue behavior, lifecycle cancellation
  state, programmatic capability-grant permission decisions bound to authenticated
  sessions, `PlanError`/redaction denials,
  active-schema extension-result primary/receipt payloads across the full
  locked callback surface, method-specific primary value families,
  receipt-category binding, worker-profile envelope JCS `result_hash` binding even for
  locked-schema primary values that cannot carry their own receipt field,
  primary receipt binding when the primary schema includes a receipt,
  and monotonic result data-class coverage. It rejects MCP/private-process substitutes,
  unpinned/latest worker-profile versions, non-stdio-first transport drift,
  human/always-grant permission substitutes, unbounded update declarations,
  archived/private schema bindings, bare method-specific result payloads,
  cross-method payloads, wrong receipt classes, unschematized primary/receipt
  payloads, unbound or forged result hashes including generic extension and
  receiptless workspace primaries, unbound primary receipts, and
  result data-class gaps. It does not prove worker process startup,
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

- Do not accept archived draft directories, downloaded zips, or MCP bridge
  draft payloads as current V1 input.
- Do not mark conformance rows proven from schema compilation alone unless the
  row explicitly says `shape_only`.
- Do not add generated structs that round-trip JSON but are never executable and
  call that the seam.
- Do not cite Codex app-server provider connectivity, `AgentRuntime` tests, or
  `FakeAgentRuntime` behavior as worker transport evidence unless the test uses
  the Leaven worker transport adapter and a child process over stdio JSON-RPC.
