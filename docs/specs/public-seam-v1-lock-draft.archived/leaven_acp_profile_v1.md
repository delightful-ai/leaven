# Leaven Agent Client Protocol Profile V1

Status: **candidate normative profile**.

Protocol pin: Agent Client Protocol **v1** (the integer `protocolVersion: 1` exchanged in `initialize`; see `docs/protocol/initialization.mdx:86-101` in the ACP source). Companion: `leaven_mcp_over_acp_profile_v1.md` (drafted in parallel) carries LM/agent/sandbox/human dispatch.

Audience: engine implementers, worker-shim authors (Python/TS SDK), Leaven CLI authors, security reviewers, anyone embedding Leaven as a service or running stage workers against it. Not user-facing documentation; users should never need to read this file.

Purpose: define how the Leaven public seam rides on top of Agent Client Protocol so that the `leaven.plan.v1`, `leaven.plan_result.v1`, `leaven.capability.v1`, `leaven.stage_payloads.v1`, `leaven.evaluation_job.v1`, and `leaven.evidence_envelope.v1` schemas reach stage workers over a real wire without inventing a bespoke RPC protocol. This document replaces the deleted `leaven.worker_protocol.v1.schema.json`.

Non-goal: redefining any schema already published in the `schemas/` bundle. Non-goal: defining LM, agent, sandbox, or human dispatch — those ride on MCP-over-ACP and are normatively defined by the companion profile.

Terminology: **MUST** is a contract; **SHOULD** is a default that may be overridden with an explicit reason; **MAY** is allowed but not assumed. Lowercase normative text follows the same convention. References to "ACP" without a version qualifier mean the v1 surface defined under `protocolVersion: 1`.

---

## 0. The judgment this profile carries

### 0.1 ACP is the transport, not the ontology

Leaven's data model, capability model, and evidence model are defined by the schema bundle. ACP supplies a JSON-RPC 2.0 wire, a session lifecycle, programmatic permission gating, and a notification channel. Nothing about ACP's IDE/coding-agent vocabulary leaks into Leaven's IR.

### 0.2 The base protocol carries the worker session; MCP-over-ACP carries LM and tool callbacks

ACP `session/new` opens a stage call. The Leaven extension methods defined here move IR ops, receipts, writes, watches, and event emissions across that session. LM completions, sandbox exec, agent runs, and human review do not ride this profile; they ride the companion MCP-over-ACP profile on the same socket.

### 0.3 Capability tokens authenticate via `authenticate`; permission flow is programmatic

The bearer token is delivered out-of-band (env var on stdio, header on HTTPS). The engine looks up the grant document by fingerprint. `session/request_permission` is reinterpreted as a programmatic capability check — never a UI prompt. The engine answers from the persisted grant. The token MUST NOT travel inside plan JSON, prompt content, `_meta`, or any other in-band envelope.

### 0.4 Backpressure is the integration layer's problem; this profile requires bounded channels

The reference ACP Rust SDK uses unbounded mpsc throughout (`src/agent-client-protocol/src/jsonrpc.rs:1231-1253, 1735, 1855`). At high notification volume a worker can OOM before the engine notices. Leaven worker shims MUST use bounded channels for outbound notifications; the engine MUST use bounded channels for inbound notifications.

### 0.5 Data-class labels travel in `_meta` on every Leaven method

Every `leaven/*` method's `params` carries `_meta.leaven_input_classes` declaring what classes flow in. The engine validates against the grant before processing. Every result that carries Leaven-managed data carries `_meta.leaven_output_classes` so the worker can track taint forward.

### 0.6 Receipts are returned on every Leaven method result

Every Leaven extension method's result includes `_meta.leaven_receipt` (an `OperationReceiptV1`) and `_meta.leaven_redactions` (when redactions occurred). The primary result payload is alongside, not nested inside, the receipt envelope.

### 0.7 The naming inversion is documented, not hidden

ACP's mental model is "IDE-client + coding-agent." Leaven uses "engine-client + worker-agent." Roles map cleanly — engine = ACP client; worker = ACP agent — but every reader of this profile MUST internalize the mapping before reading ACP source materials.

### 0.8 Pin to ACP v1; track v2 but do not depend on it

ACP v2 has landed (May 2026). RFDs are in flight for HTTP transport, session-fork, MCP-over-ACP standardization, custom LLM endpoints, and elicitation. Leaven v1 ships against ACP v1 and the named v1 capability surface. Adopting v2 features is a Leaven v1.x decision per feature, not a transport version bump.

---

## 1. Scope

This profile covers:

- Transport bindings under which ACP carries Leaven stage-worker traffic.
- Authentication of capability tokens via ACP `initialize`/`authenticate`.
- Session lifecycle for stage calls (`session/new`, `session/load`, `session/cancel`, `session/close`).
- Programmatic answering of `session/request_permission` against capability grants.
- Streaming server-to-client progress via `session/update` and the backpressure contract.
- The closed set of `leaven/*` JSON-RPC methods workers MAY invoke on the engine over an active session.
- Data-class labeling, receipt return shape, and error model for those methods.
- Heartbeat, timeout, capability expiry, and graceful shutdown.

This profile does not cover:

- LM, agent, sandbox, or human dispatch (see the companion MCP-over-ACP profile).
- Pre-spawn worker discovery, registries, or marketplaces.
- IDE-style UX (slash commands, agent plans, terminals, NES).
- ACP v2 features (session fork, elicitation, LLM provider negotiation, HTTP/WebSocket transport).
- The internal Rust shape of worker SDKs.

---

## 2. ACP version pin

The profile is bound to ACP `protocolVersion: 1`.

The worker MUST send `protocolVersion: 1` in its `initialize` request unless and until this profile is republished against a later ACP version. The engine MUST refuse a worker that proposes a different version with a JSON-RPC error using `code: -32602` (`Invalid params`) and a human-readable message naming the required version, per ACP's `initialize` negotiation rules (`docs/protocol/initialization.mdx:94-101`).

The following ACP v1 baseline methods are in scope and MUST be supported by both sides:

- `initialize` (`src/v1/agent.rs:INITIALIZE_METHOD_NAME`).
- `authenticate` (`src/v1/agent.rs:AUTHENTICATE_METHOD_NAME`).
- `session/new` (`src/v1/agent.rs:SESSION_NEW_METHOD_NAME`).
- `session/cancel` notification (`src/v1/agent.rs:SESSION_CANCEL_METHOD_NAME`).
- `session/update` notification (`src/v1/client.rs` outbound notifications).
- `session/request_permission` (`src/v1/client.rs:SESSION_REQUEST_PERMISSION_METHOD_NAME`).

The following ACP v1 methods MAY be supported and SHOULD be advertised via standard ACP capabilities when present:

- `session/load` — gated by `agentCapabilities.loadSession`. Used for stage-call resume.
- `session/close` — gated by `agentCapabilities.sessionCapabilities.close`. Used for graceful shutdown.
- `logout` — gated by `agentCapabilities.auth.logout`. Used for capability revocation handshake.

ACP v1 methods explicitly **out of scope** for this profile, even when ACP advertises them:

- `session/prompt`. Leaven workers do not receive natural-language prompts; the stage payload IS the prompt analogue.
- `fs/read_text_file`, `fs/write_text_file`. Workspace reads go through `leaven/workspace.*`. Workspace mutation is staged via `leaven/proposal.submit_batch` and `leaven/workspace.materialize`.
- `terminal/*`. Subprocess execution is dispatched via MCP-over-ACP `sandbox.exec`, not via ACP terminals.
- `session/set_mode`, `session/set_model`, `session/set_config_option`, `session/list`. Stage workers do not negotiate modes/models in band.
- `nes/*`, `document/*`. Editor-style next-edit-suggestion is not a Leaven concern.

ACP v2 features (session fork, elicitation, multi-LLM endpoint config) MUST NOT be relied on by v1 implementations of this profile. A worker MAY advertise support for v2 features via `_meta` capability hints, but the engine MUST treat any v2-only behavior as absent.

---

## 3. The naming inversion

ACP names the JSON-RPC sides "client" and "agent." Leaven names them "engine" and "worker." The mapping is exact and load-bearing:

| ACP role | Leaven role | Who owns process lifetime | Who answers `session/request_permission` |
|----------|-------------|---------------------------|------------------------------------------|
| Client   | Engine      | Engine spawns worker      | Engine                                   |
| Agent    | Worker      | Engine spawns worker      | n/a (engine answers)                     |

Concretely:

- The **Leaven engine** acts as the **ACP client**. It calls `initialize`, `authenticate`, `session/new`, `session/load`, `session/cancel`, `session/close`. It receives `session/update` notifications. It receives `session/request_permission` requests and answers them programmatically from the capability grant.
- The **Leaven worker** acts as the **ACP agent**. It responds to `initialize` and the session-lifecycle methods. It emits `session/update` notifications during stage execution. It MAY issue `session/request_permission` to ask the engine to gate-check a privileged action, but in this profile the more common pattern is that the worker calls a `leaven/*` method directly and the engine performs the capability check inline.

This inversion is the largest documentation hazard in this profile. Implementers reading ACP docs MUST mentally swap "client"→"engine" and "agent"→"worker" or they will route message handlers backwards.

Worker shims MAY (and SHOULD) refer to ACP roles by ACP's vocabulary in their source code (e.g., a Rust shim's `impl acp::Agent for LeavenWorker` is correct), but user-facing names, log lines, error messages, and trace spans MUST use Leaven vocabulary ("engine"/"worker").

---

## 4. Transport bindings

ACP defines stdio as the canonical transport (`docs/protocol/transports.mdx:17-42`). This profile pins three transports for Leaven, with stdio canonical.

### 4.1 stdio (canonical)

Engine spawns worker as a child process.

- Engine MUST write only valid newline-delimited JSON-RPC 2.0 frames to the worker's `stdin`.
- Worker MUST write only valid newline-delimited JSON-RPC 2.0 frames to its `stdout`.
- Frames MUST be UTF-8. Frames MUST NOT contain embedded `\n`. Each frame ends with exactly one `\n`.
- Worker MAY write free-form UTF-8 logging to `stderr`. Engine MAY capture, forward, or discard `stderr`.
- Engine MUST initiate shutdown by closing the worker's `stdin`, optionally preceded by `session/cancel` notifications.
- Worker MUST exit promptly after `stdin` close.

Per-frame size: the engine SHOULD reject frames larger than 16 MiB by default; the worker SHOULD do the same; both MAY raise the cap via configuration, but the limit MUST be finite. Frames carrying large blob content (workspace digests, transcripts) MUST cite by `ReceiptRef` rather than inline.

### 4.2 Unix domain socket

For long-lived worker pools and shared-host deployments.

- The engine listens on a Unix domain socket path. Filesystem permissions on the socket are the access boundary.
- The worker connects, then exchanges identical newline-delimited JSON-RPC 2.0 frames as in stdio.
- The same framing and frame-size rules from 4.1 apply.
- The engine MUST verify the worker's peer credentials (`SO_PEERCRED` or platform equivalent) when capability-token verification relies on Unix-credentialed trust.

### 4.3 HTTPS

For network-deployed engines (remote workers, multi-tenant cloud, cross-host pools).

- Each ACP session MUST ride a single long-lived bidirectional connection. WebSocket over TLS is the recommended substrate; HTTP/2 streaming is acceptable; HTTP/1.1 long-poll is forbidden.
- Frames MUST carry the same JSON-RPC 2.0 envelope shape used on stdio. Frame framing is the substrate's responsibility (WebSocket message boundaries; HTTP/2 DATA frame boundaries).
- TLS MUST authenticate the engine to the worker. The engine MAY require mTLS for worker authentication.
- The capability token MUST travel as a request header on the initial upgrade (4.4.3), never as part of the JSON-RPC payload.

### 4.4 Capability-token credential delivery

The capability token is the bearer credential identifying the worker to the engine. Its delivery is transport-specific. It MUST NOT appear in plan JSON, prompt content, `session/update` notifications, `session/new` params, or any Leaven extension method params, including their `_meta` fields. The token's grant document fingerprint (`capability_fingerprint` in `leaven.capability.v1`) MAY travel in-band; the bearer secret MUST NOT.

#### 4.4.1 stdio delivery

The engine MUST inject the following environment variables on worker spawn:

- `LEAVEN_CAPABILITY_TOKEN` — the bearer secret. Opaque string.
- `LEAVEN_ENDPOINT` — informational, e.g., `stdio` or `unix:///run/leaven/engine.sock`; useful when workers can connect to alternative transports.
- `LEAVEN_GRANT_FINGERPRINT` — the `CapabilityFingerprint` for the persisted grant document. Used by the worker to pin which grant it expects to be authenticated against.

The worker SHOULD scrub `LEAVEN_CAPABILITY_TOKEN` from its environment immediately after reading it, so that child processes inherit only the fingerprint, not the bearer.

#### 4.4.2 Unix socket delivery

Same env-var protocol as stdio. The peer credential check from 4.2 is an additional defense.

#### 4.4.3 HTTPS delivery

The capability token MUST travel as `Authorization: Bearer <token>` on the initial HTTP upgrade request. It MUST NOT travel in subsequent JSON-RPC frames. The engine MUST close the connection if the token rotates mid-session unless an explicit attenuated rotation flow is negotiated (out of scope for v1).

The `LEAVEN_GRANT_FINGERPRINT` SHOULD travel as an additional header `X-Leaven-Grant-Fingerprint` for client-side sanity checking.

---

## 5. Authentication flow

The authentication flow is the bridge between an opaque bearer token and the persisted `leaven.capability.v1` grant document.

### 5.1 Initialize

The worker MUST issue `initialize` as its first JSON-RPC request, before any other call. Per ACP, the `params` object carries `protocolVersion: 1`, optional `clientCapabilities`, and optional `clientInfo`. The worker SHOULD also include `_meta.leaven_profile = "leaven.acp.v1"` so the engine can fail fast if the worker is misconfigured.

The engine MUST respond with:

- `protocolVersion: 1`.
- `agentCapabilities` declaring which ACP capabilities the engine supports (e.g., `loadSession`, `sessionCapabilities.close`, `auth.logout`).
- `authMethods` advertising at minimum one method whose `id` is `"leaven.capability.v1"`. The method `type` MUST be `agent` (ACP default, per `docs/protocol/authentication.mdx:67-77`).
- `_meta.leaven_acp_profile_version = "1"` so the worker can pin to this profile.

The engine MUST NOT accept any `session/*` request before `authenticate` succeeds.

### 5.2 Authenticate

The worker MUST issue `authenticate` with `params.methodId = "leaven.capability.v1"`. Per ACP, the `params` object MAY carry `_meta`. In this profile the worker MUST include:

- `_meta.leaven_capability_token` — the bearer string read from `LEAVEN_CAPABILITY_TOKEN`. This is the **only** place the bearer secret appears in-band, and the engine MUST scrub it from any post-authentication audit log entry.
- `_meta.leaven_grant_fingerprint` — the expected `CapabilityFingerprint`, supplied for sanity checking only. The engine MUST verify the bearer first and the fingerprint match second; mismatch is a hard error.

Token verification semantics by transport:

- **stdio**: kernel-credentialed parent/child relationship is the trust. The engine MUST look up the bearer in its token store; the token store MAY be in-memory for ephemeral runs and MUST be durable for runs producing graph writes. Signed tokens are NOT required.
- **Unix socket**: same as stdio plus the peer credential check from 4.2.
- **HTTPS**: the engine MUST verify the bearer against its token store and MUST verify TLS. The token store SHOULD be backed by HMAC or a key-derivation path so that token-store compromise does not immediately yield all secrets in plaintext; mTLS MAY replace HMAC for service-to-service deployment.

On success the engine MUST respond with an empty `result: {}` plus `_meta` carrying a non-sensitive grant summary:

- `_meta.leaven_grant_fingerprint` — confirmed fingerprint.
- `_meta.leaven_policy_fingerprint` — the `PolicyFingerprint` bound to the grant.
- `_meta.leaven_subject` — `SubjectV1` projection (no bearer-derived fields).
- `_meta.leaven_expires_at` — the grant's `expires_at` timestamp.
- `_meta.leaven_audience` — the grant's audience array.

On failure the engine MUST respond with a JSON-RPC error using the closed Leaven error code set defined in §14. Typical codes: `token_invalid`, `token_expired`, `capability_denied`.

The worker MUST NOT retry `authenticate` on failure beyond a single resubmission with a freshly resolved token; repeated failures MUST end with the worker exiting non-zero.

### 5.3 What MUST NOT be persisted

Run artifacts, plan results, receipts, logs, traces, and audit records MUST cite the `capability_fingerprint` and a non-sensitive grant summary. They MUST NOT cite the bearer secret, `LEAVEN_CAPABILITY_TOKEN`'s value, or any signed-token payload derivable to the bearer.

This rule is restated from `leaven_public_seam_v1_lock_spec.md` §6.1 and §14 ("Never persist bearer token secrets.").

### 5.4 Logout

If the engine advertised `agentCapabilities.auth.logout`, the worker MAY issue `logout` before exit. The engine MUST treat `logout` as an explicit signal that no further sessions on this connection should succeed; it MUST close any active sessions (mirroring `session/cancel`) and MUST mark the bearer as revoked for the connection's lifetime.

`logout` is not required. Most stage workers will exit by closing `stdin`/connection without logout.

---

## 6. Session lifecycle

Each Leaven stage call corresponds to one ACP session. Sessions are not Leaven runs; one Leaven run produces many stage calls.

### 6.1 session/new

The worker MUST issue `session/new` for each stage call. Per ACP, `params` carries `cwd: PathBuf` (absolute) and `mcpServers: McpServer[]`. In this profile:

- `cwd` MUST be set to an absolute path on a filesystem the engine considers part of the worker's execution profile. For stdio workers, this is typically the worker's process cwd.
- `mcpServers` is reserved for engine-supplied MCP server connections that the worker should attach to. The companion MCP-over-ACP profile uses this list to register the engine itself as an MCP server reachable over the ACP socket.
- `params._meta.leaven_stage_call_id` — the `StageCallId` that this session is bound to. MUST be present. MUST match the `stage_call_id` in the authenticated grant's `subject` (when `subject.kind == "stage_call"` or `"evaluation_stage_call"`).
- `params._meta.leaven_role` — the `StageRole` from the grant. Engine MUST cross-check.
- `params._meta.leaven_payload_ref` — `InfoRef` pointing to the stage payload (typed by `leaven.stage_payloads.v1`). The engine MAY also accept an inline `_meta.leaven_payload` for small payloads but SHOULD reject payloads larger than 256 KiB inline.

The engine MUST respond with a `NewSessionResponse`:

- `sessionId` — a fresh `SessionId`.
- `_meta.leaven_session_binding` — restated `stage_call_id`, `role`, and `evaluation_request_id` (when applicable) so the worker can pin against drift.

The session is bound to the capability token's `stage_call_id` scope. Opening a second `session/new` whose `_meta.leaven_stage_call_id` differs from the authenticated subject MUST be rejected with `capability_denied`.

Multiple sessions per process are permitted only when the token's subject is `service` or `operator`. For `stage_call`/`evaluation_stage_call` subjects, the engine MUST limit the connection to exactly one active session at a time.

### 6.2 session/load

When `agentCapabilities.loadSession` is advertised, the worker MAY issue `session/load` for stage-call resume. Per ACP this triggers conversation replay; in this profile the engine MUST replay only `session/update` notifications carrying Leaven-domain events (e.g., previously emitted `leaven/event.emit` echoes, watch deliveries that were not yet acknowledged).

The engine MUST verify that:

- The `sessionId` in `params` was previously bound to the same `stage_call_id` as the current capability subject.
- The base graph revision at resume time is compatible with the prior session's reads (the engine MAY refuse with `precondition_failed` if `since_revision` semantics cannot be honored).

If either check fails the engine MUST respond with the typed error and not replay.

### 6.3 session/cancel

The worker MUST honor `session/cancel` notifications by:

- Aborting any in-flight `leaven/*` request whose result has not been emitted.
- Draining bounded notification channels (see §9.4).
- Refusing all new `leaven/*` requests with `cancelled`.
- Responding to any pending `session/request_permission` with `outcome.outcome = "cancelled"` per `docs/protocol/tool-calls.mdx:168-180`.

The engine MUST issue `session/cancel` on any of:

- Operator cancellation.
- Capability token revocation mid-session.
- Heartbeat timeout (§15).
- Run-level cancellation.

`session/cancel` is one-way (no response). The engine MUST treat the session as observationally cancelled the moment the notification is sent, but MUST continue accepting `session/update` notifications and committed graph writes already in flight, until the worker either exits or the connection closes.

### 6.4 session/close

When `agentCapabilities.sessionCapabilities.close` is advertised, the engine MAY issue `session/close` for graceful shutdown of a stage call without tearing down the worker process. Per ACP (`docs/protocol/session-setup.mdx:251-307`), `session/close` is semantically `session/cancel` plus resource release.

In this profile, after `session/close` succeeds the engine MUST:

- Drop the session's capability scope (no more `leaven/*` methods accepted for this `sessionId`).
- Preserve the worker process for further authenticated sessions if the token's subject permits it (typically `service` subjects).

ACP v1 may not advertise `sessionCapabilities.close` at all in some implementations. When absent, the engine MUST achieve the same effect by sending `session/cancel` and allowing the worker to either start a new session or exit.

### 6.5 Session-to-token scope

A session's authority is the intersection of:

- The capability grants attached to the authenticated bearer.
- The `stage_call_id` scope encoded in the session's `_meta.leaven_stage_call_id`.
- The `evaluation_request_id` scope encoded in the grant's subject (when subject is `evaluation_stage_call`).

The engine MUST evaluate every `leaven/*` method's authorization against the **session's bound scope**, not against the bearer's raw grants. This prevents a worker that legitimately holds a multi-stage `service` token from accidentally widening its scope by opening a session whose `_meta.leaven_stage_call_id` does not match the legitimate request.

---

## 7. Permission flow as programmatic capability check

ACP's `session/request_permission` is normally a user-facing UI prompt. The spec explicitly allows automation (`docs/protocol/tool-calls.mdx:166`: "Clients **MAY** automatically allow or reject permission requests according to the user settings."). This profile uses that allowance fully.

### 7.1 When the worker issues request_permission

The worker MAY issue `session/request_permission` when it needs the engine to gate-check an action that does not naturally route through a `leaven/*` method. Examples: a worker that is about to call an external BYO LM provider (in trusted-local profile) wants to record that the engine acknowledged the BYO call; a worker that intends to write a workspace file via its own filesystem capability wants the engine to confirm path policy.

In all other cases the worker SHOULD call a `leaven/*` method directly, because those methods perform the capability check inline and emit a receipt.

### 7.2 Reinterpretation of toolCall

Per ACP, `session/request_permission` `params` carries `toolCall: ToolCallUpdate` (`docs/protocol/tool-calls.mdx:142-148`). In this profile that field is reinterpreted as a **capability check request** with the following extensions in `_meta`:

- `params.toolCall.toolCallId` — a worker-generated `ToolCallId` used to correlate the permission decision with later `session/update` notifications. Required by ACP.
- `params.toolCall.title` — a short human-readable description; the engine MAY ignore this. Required by ACP.
- `params.toolCall.kind` — set to `"other"`. The ACP kinds (`read`/`edit`/`delete`/`move`/`search`/`execute`/`think`/`fetch`/`other`) are advisory; Leaven uses `"other"` and overrides via `_meta.leaven_action`.
- `params.toolCall._meta.leaven_action` — REQUIRED. A grant-action path string drawn from the closed set (`graph.read`, `case.read`, `workspace.read`, `workspace.materialize`, `sandbox.exec`, `lm.complete`, `agent.run`, `human.review`, `proposal.submit`, `proposal.apply`, `assessment.submit`, `evaluation.request`, `watch.start`, `extension.read`, `extension.call`, `extension.write`).
- `params.toolCall._meta.leaven_args` — REQUIRED. A structured constraints object whose shape mirrors the matching `Grant.constraints` schema. The engine evaluates this against the grant.
- `params.toolCall._meta.leaven_input_classes` — REQUIRED for any action carrying data flow. The data classes the worker asserts flow into the action.
- `params._meta.leaven_stage_call_id` — REQUIRED. Echoed for engine correlation.

`params.options` is also REQUIRED by ACP. The worker MUST present two options:

```
options: [
  { optionId: "approve", name: "Approve", kind: "allow_once" },
  { optionId: "deny",    name: "Deny",    kind: "reject_once" }
]
```

The `kind` values `allow_always`/`reject_always` MUST NOT appear; capability decisions in this profile are scoped to the session, not remembered across sessions.

### 7.3 Engine response

The engine MUST evaluate `_meta.leaven_action` and `_meta.leaven_args` against the bound grant. The response follows ACP `RequestPermissionOutcome` shape (`docs/protocol/tool-calls.mdx:151-186`):

- **Approved**: `result.outcome = { outcome: "selected", optionId: "approve" }`. The engine MAY include `result._meta.leaven_grant_match` echoing the matched grant's `action` and pertinent constraint summary, for audit. The engine MUST include `result._meta.leaven_receipt` — a `ReceiptRef` to the capability check receipt.
- **Denied**: `result.outcome = { outcome: "selected", optionId: "deny" }`. The engine MUST include `result._meta.leaven_redaction` — a `Redaction` (from `common.schema.json`) whose `reason` names why the action was refused. The engine MUST include `result._meta.leaven_receipt`. The engine SHOULD also include `result._meta.leaven_error_code` from the closed set in §14 (typically `capability_denied`, `data_class_violation`, `budget_exceeded`, `provider_policy_denied`).
- **Cancelled**: `result.outcome = { outcome: "cancelled" }` is reserved for the case where the engine sent `session/cancel` during the permission round-trip. The engine MUST NOT use `cancelled` for any other denial reason. The worker MUST treat `cancelled` as session termination, not as a per-action denial.

### 7.4 What request_permission MUST NOT carry

- Raw bearer tokens.
- Hidden target data, hidden split internals, or any `case.target` payload.
- Plan IR fragments. Plans are evaluated via `leaven/graph.query` and friends, not via permission requests.
- LM dispatch parameters. Those ride the companion MCP-over-ACP profile.

---

## 8. Streaming server-to-client updates

ACP `session/update` is a one-way notification from agent to client carrying progress, partial output, tool-call status, plan updates, etc. In this profile the worker uses it to push Leaven-domain events to the engine without blocking on a request/response round trip.

### 8.1 What updates carry

The worker MAY send `session/update` to the engine carrying any of:

- `_meta.leaven_progress` — periodic progress reports for long-running stage calls (e.g., "case 37 of 240 evaluated"). Non-graph.
- `_meta.leaven_log` — structured log events (level, message, fields). Non-graph.
- `_meta.leaven_event_pending` — notification that the worker intends to emit a graph event via `leaven/event.emit`. The actual graph write travels via the method, not the notification.
- `_meta.leaven_watch_delivery` — outbound watch deliveries (see §10's `leaven/watch.next`).

Notifications MUST NOT carry committed graph state, receipts that have not been issued by the engine, or capability secrets.

The `update.sessionUpdate` ACP enum (`agent_message_chunk`, `tool_call`, `tool_call_update`, `plan`, `user_message_chunk`, ...) is largely irrelevant to Leaven and MAY be set to `"agent_message_chunk"` with empty `content` when the worker only needs to carry `_meta` payloads. The engine MUST tolerate any `update.sessionUpdate` discriminator.

### 8.2 Bounded mpsc requirement

Worker shims MUST use bounded channels for outbound `session/update` traffic.

- The reference ACP Rust SDK (`src/agent-client-protocol/src/jsonrpc.rs:1231-1253`) uses `mpsc::unbounded` for the outgoing message queue. A naive worker shim built on that SDK will accumulate unbounded notifications under engine slowness and OOM. Worker shims built on the reference SDK MUST wrap the outbound notification path in a bounded queue before handing messages to the SDK.
- The default bound SHOULD be configurable. A reasonable default is 1024 messages or 4 MiB cumulative payload, whichever is smaller.
- When the bound fills the worker MUST apply one of the following policies, chosen at shim-init time and stable for the session's lifetime:
  - `block` — caller blocks until capacity returns. Preferred for stage workers whose semantics depend on every update being delivered.
  - `drop_oldest` — discard the head of the queue. Permitted for purely advisory `_meta.leaven_progress` and `_meta.leaven_log` updates; FORBIDDEN for `_meta.leaven_event_pending` or `_meta.leaven_watch_delivery`.
  - `disconnect` — close the session with `cancelled`. Acceptable for batch evaluators with strict latency budgets.
- The worker MUST log (via stderr) the bound, the policy, and any overflow occurrences. The engine MAY surface those via the run's event log if the worker also calls `leaven/event.emit` with an overflow event.

The engine MUST also bound its inbound notification queue with a documented policy, defaulting to `block`.

### 8.3 Credit-based flow control

A worker MAY negotiate credit-based flow control by including in its `session/new` `_meta` field `leaven_flow_control: { mode: "credit", initial_credit: 256 }`. If the engine acknowledges by echoing the field in its `NewSessionResponse._meta`, both sides MUST observe the credit:

- The engine periodically grants additional credit via `session/update` notifications carrying `_meta.leaven_credit_grant: <integer>` from engine to worker. (Engine-to-worker `session/update` is not normally used in ACP, but credit grants ride here as an exception inside the agreed flow-control extension.)
- The worker MUST NOT send more than `outstanding_credit` notifications.
- Credit consumption applies to **all** outbound notifications, including watch deliveries.

Credit-based flow control is OPTIONAL for v1. When absent, the bounded-channel rule in 8.2 is the only backpressure mechanism.

### 8.4 Ordering

Within a session, `session/update` notifications from the worker MUST be delivered to the engine in send order. Notifications carrying `_meta.leaven_event_pending` for events that will be committed via `leaven/event.emit` MUST be sent **before** the corresponding `leaven/event.emit` request returns, so that engine observers see the announcement and the commit in causal order.

---

## 9. Leaven extension methods

Each method below is a JSON-RPC request issued **by the worker to the engine** during an authenticated session. Method names use the `leaven/` namespace prefix to coexist with ACP-namespaced methods (`session/*`, `fs/*`, `terminal/*`). The methods are not custom-extension methods in the ACP `_` sense (per `docs/protocol/extensibility.mdx:43`); they are domain methods owned by this profile and registered via `_meta.leaven_acp_profile_version`.

For each method this section specifies:

- The JSON-RPC method name.
- The shape of `params` (referencing existing schemas; not redefining).
- The required capability action (the matching `GrantV1` action string).
- Data-class label handling on inputs and outputs.
- Receipt return shape.

Every method's `params` carries `_meta.leaven_input_classes: string[]` declaring the data classes flowing into the call. Every method's `result` carries `_meta.leaven_receipt: OperationReceiptV1` and MAY carry `_meta.leaven_redactions: Redaction[]` and `_meta.leaven_output_classes: string[]`. These are not restated per method.

Every method response MUST include `_meta.leaven_capability_fingerprint` echoing the grant fingerprint used to authorize the call, so that downstream audit/replay can pin the response to the exact authority decision.

### 9.1 Graph reads

#### leaven/graph.query

Evaluate a graph query expression.

- `params.expr` — an `ExprV1` from `leaven.plan.v1` of `kind: "graph_query"`. The worker SHOULD pass the inner shape directly to avoid the double-wrapping audit issue noted in `COMPREHENSIVE_DESIGN_PASS_NOTES.md` §5.3 issue 28.
- `params.consistency` — optional `Consistency` from `leaven.plan.v1`. Defaults to `latest_at_start` resolved at session open.
- Capability action: `graph.read`.
- Result: `result.value` — a typed query result mirroring the projection requested. `result._meta.leaven_receipt.kind == "query"`.

#### leaven/case.load

Read a case under the bound case-read grant.

- `params.case` — a `CaseRef` or `CaseId`.
- `params.include` — array drawn from `["input", "target", "metadata"]`. Each requested field MUST be permitted by the grant's `constraints.fields`.
- `params.consistency` — optional.
- Capability action: `case.read`.
- Result: a `CaseRecord` projection. `result.value.target` is present only when both `target` was requested AND the grant allows it.

#### leaven/case.input

Read only the runner-visible case input. Convenience for runners that MUST NOT see targets.

- `params.case` — required.
- Capability action: `case.read` with `constraints.fields ⊇ ["input"]`.
- Result: a `CaseInput` projection.

#### leaven/case.target

Read only the case target. Reserved for scorer/evaluator stages.

- `params.case` — required.
- Capability action: `case.read` with `constraints.fields ⊇ ["target"]` AND `constraints.purpose ∈ ["scorer", "evaluator"]`.
- Result: a `CaseTarget` projection. The receipt is marked `target_derived = true`.

#### leaven/case.metadata

Read only case metadata under the metadata projection policy.

- Capability action: `case.read` with `constraints.fields ⊇ ["metadata"]`.

### 9.2 Workspace reads

The following are direct mappings of `WorkspaceQueryV1` variants from `leaven.plan.v1`. Each requires `workspace.read` action with the corresponding `ops` constraint.

- **leaven/workspace.snapshot** — `params.workspace: WorkspaceRef`. Returns a snapshot handle pinned at a `snapshot_fingerprint`. `ops ⊇ ["snapshot"]`.
- **leaven/workspace.list** — `params.workspace`, `params.path`, optional `params.glob`. `ops ⊇ ["list"]`.
- **leaven/workspace.read_file** — `params.workspace`, `params.path`, optional `params.byte_range`. `ops ⊇ ["read_file"]`. Result content is labeled with data classes declared at grant time on `WorkspaceReadGrant.constraints.data_classes`; the worker MUST treat the response data classes (in `_meta.leaven_output_classes`) as authoritative.
- **leaven/workspace.stat** — `params.workspace`, `params.path`. `ops ⊇ ["stat"]`.
- **leaven/workspace.digest** — `params.workspace`, `params.paths`. `ops ⊇ ["digest"]`. The hash algorithm is the engine-pinned canonical hash; the worker MUST NOT assume SHA-256 specifically.
- **leaven/workspace.git_log** — `params.workspace`, optional `params.path_filter`, optional `params.max_count`. `ops ⊇ ["git_log"]`.
- **leaven/workspace.git_diff** — `params.workspace`, `params.from`, `params.to`. `ops ⊇ ["git_diff"]`.
- **leaven/workspace.capture_artifacts** — `params.workspace`, `params.specs`. `ops ⊇ ["capture_artifacts"]`.

### 9.3 Workspace materialization

#### leaven/workspace.materialize

Open a workspace handle for downstream sandbox exec, agent run, or proposal writeback.

- `params.candidate` — `CandidateRef`.
- `params.mode` — one of `read_only_snapshot | mutable_eval_workspace | agent_workspace`.
- `params.roots` — optional `string[]` restricting which artifact roots to materialize.
- Capability action: `workspace.materialize` with matching `constraints.modes`.
- Result: `result.handle: WorkspaceHandle` (typed as `_meta.leaven_value_kind = "workspace_handle"`). Receipt kind `call`, `call_kind: "workspace_materialize"`.
- Lifetime: the handle is valid for the lifetime of the session unless explicitly released via a future `leaven/workspace.release` (reserved; not in v1). On `session/close` or `session/cancel` the engine MUST release the handle.

### 9.4 Proposals

#### leaven/proposal.submit_batch

Submit a proposal batch. Append-only graph mutation intent; engine commits via `RunContext`.

- `params.batch` — a `SubmitProposalBatchWriteV1` from `leaven.plan.v1`. The worker builds this in host language and submits as a value.
- `params.preconditions` — optional `PreconditionV1[]`.
- Capability action: `proposal.submit` with matching `constraints.effects`, `allowed_targets`, `allowed_surfaces`, `artifact_schemas`, `change_schemas`.
- Result: `result.batch_id: ProposalBatchId`, `result.proposal_ids: ProposalId[]`. Receipt kind `write`.

#### leaven/proposal.apply

Apply a previously submitted proposal batch.

- `params.batch` — `ProposalBatchId`.
- `params.stale_policy` — optional; matches `ProposalApplyGrant.constraints.stale_policy`.
- Capability action: `proposal.apply` (which is a **separate** grant from `proposal.submit`; submit MUST NOT imply apply).
- Result: `result.applied: boolean`, `result.committed_revision: GraphRevision`, `result.population_event_ids: EventId[]`.

### 9.5 Assessments

#### leaven/assessment.submit

Submit assessments under the evaluation request scope.

- `params.assessments` — `AssessmentWriteV1[]` from `leaven.plan.v1`.
- `params.evaluation_request_id` — REQUIRED. MUST equal the grant's `subject.evaluation_request_id` for `evaluation_stage_call` subjects.
- Capability action: `assessment.submit` with matching `constraints.assessment_shapes`, `granularity`, `allowed_candidates`, `allowed_cases`, `purpose`, `evidence_visibility_allowed`.
- Result: `result.assessment_ids: AssessmentId[]`. Receipt kind `write`.

### 9.6 Evaluation requests

#### leaven/evaluation.request

Request a downstream evaluation job (typically issued by an optimizer-shaped worker).

- `params.request` — a `RequestEvaluationWriteV1` from `leaven.plan.v1` (note: this op currently has loose typing per `COMPREHENSIVE_DESIGN_PASS_NOTES.md` §5.3 issue 19; the engine SHOULD enforce shape via `evaluation_job.v1`).
- Capability action: `evaluation.request` with matching `constraints.shapes`, `sets`, `hidden_partitions`.
- Result: `result.evaluation_request_id: EvaluationRequestId`.

### 9.7 Events

#### leaven/event.emit

Emit a run event (population event, run-event, scorer/evaluator beacon).

- `params.event` — an `EmitRunEventWriteV1` from `leaven.plan.v1`.
- Capability action: at least one grant matching the event payload's `namespace` (typically an `ExtensionGrant` or an implicit `events.emit` reserved for the worker's role).
- Result: `result.event_id: EventId`. Receipt kind `write`.

The worker SHOULD send a corresponding `session/update` notification carrying `_meta.leaven_event_pending` immediately before calling `leaven/event.emit`, so engine observers see the announcement-then-commit causal order (see §8.4).

### 9.8 Watches

Watches are sibling protocol objects per the lock spec §12. This profile carries the wire methods.

#### leaven/watch.start

Start a watch subscription bound to this session.

- `params.watch` — a `leaven.watch.v1` request object.
- Capability action: `watch.start` with matching `constraints`.
- Result: `result.watch_id: WatchId`, `result.cursor: WatchCursor`.

#### leaven/watch.next

Pull the next delivery batch from a watch.

- `params.watch_id` — `WatchId`.
- `params.cursor` — current cursor.
- `params.max_items` — optional. SHOULD respect the grant's `max_backlog`.
- Result: `result.deliveries: WatchDelivery[]`, `result.next_cursor: WatchCursor`, `result.lagging: boolean`.

Watch deliveries MAY ALSO be pushed via `session/update` notifications with `_meta.leaven_watch_delivery` for low-latency consumers; the bounded-channel and credit-based flow-control rules of §8.2 and §8.3 apply.

#### leaven/watch.ack

Acknowledge processing of delivered events up to a cursor.

- `params.watch_id`, `params.cursor`.

#### leaven/watch.cancel

Cancel a watch.

- `params.watch_id`.
- The engine MUST release the watch on session end automatically; explicit `watch.cancel` is the worker's signal that the watch is no longer needed.

### 9.9 Receipt fetch

#### leaven/plan_result.fetch

Retrieve a full `OperationReceiptV1` body and any embedded values by `ReceiptRef`.

- `params.receipt` — `ReceiptRef`.
- Result: `result.receipt: OperationReceiptV1`, optionally `result.value` (for query receipts) or `result.charges: ChargeReceiptV1[]`.
- This is a read against the engine's receipt store; it requires no grant beyond session authentication, but the engine MAY enforce that the requested receipt was produced under the same `capability_fingerprint` (or an attenuated descendant).

---

## 10. LM, agent, sandbox, and human dispatch are NOT in this profile

The following calls from `leaven_public_seam_v1_lock_spec.md` §11 are routed via the companion **`leaven_mcp_over_acp_profile_v1.md`**, not via this profile:

- `lm.complete` (and any provider-specific LM dispatch).
- `agent.run`.
- `sandbox.exec`.
- `human.review`.

The shape rationale is that LM/agent/sandbox/human dispatch is sampling-shaped (the engine acts on behalf of the worker, executing a costful operation with provider lowering, cache, budget, replay). MCP's `sampling/create_message` primitive is the natural fit. ACP v1 does not provide a sampling primitive natively, so the engine exposes itself as an MCP server reachable over the ACP socket (registered via `session/new` `params.mcpServers`), and the worker dispatches LM/agent/sandbox/human calls as MCP tool calls. The capability token authenticated via this profile's `authenticate` step gates every MCP tool call exactly as it gates every `leaven/*` method.

Workers MUST NOT implement `lm.complete` as a Leaven extension method on this profile. The engine MUST refuse `leaven/lm.complete` and similar method names with `code: -32601` (`Method not found`).

---

## 11. Data-class labels on the wire

Every `leaven/*` method's `params._meta.leaven_input_classes: string[]` declares the data classes (from the closed enum in `leaven_public_seam_v1_lock_spec.md` §6.7 plus `x.*` extensions) that flow into the call. The engine validates these against:

- The grant's `allowed_input_classes` — the set of classes permitted to flow in. If `_meta.leaven_input_classes ⊄ allowed_input_classes`, the engine MUST refuse with `data_class_violation`.
- The grant's `forbidden_input_classes` — the set of classes denied even if otherwise allowed elsewhere. If `_meta.leaven_input_classes ∩ forbidden_input_classes ≠ ∅`, the engine MUST refuse with `data_class_violation`.

Workers MUST NOT understate input classes. A worker that knows it is passing `case.target`-tainted data into a call MUST declare `case.target` in `_meta.leaven_input_classes`. The engine MAY perform additional taint inference (e.g., on values fetched via `leaven/case.target` whose `_meta.leaven_output_classes` carried `case.target`), but the worker's declaration is the primary contract; the inference is defense-in-depth.

For results, `result._meta.leaven_output_classes: string[]` declares the data classes the result content carries forward. Workers MUST treat the response labels as authoritative when threading taint through subsequent calls.

Propagation rules (per `COMPREHENSIVE_DESIGN_PASS_NOTES.md` §4.7) are normatively the union over inputs: a value derived from multiple labeled inputs carries the union of their data classes. Templates and extracts MUST forward labels of every referenced variable. Aggregations carry the union of all aggregated items' labels. `refs_from_result` produces InfoRefs whose dereferenced projections carry the labels declared at projection time. Workers MUST follow these rules when constructing later calls.

---

## 12. Receipt return shape

Every `leaven/*` method result MUST carry `_meta.leaven_receipt`. The receipt's `OperationReceiptV1` discriminator (`query | call | write`) depends on the method:

- `leaven/graph.query`, `leaven/case.*`, `leaven/workspace.{snapshot,list,read_file,stat,digest,git_log,git_diff,capture_artifacts}`, `leaven/plan_result.fetch` → `kind: "query"`.
- `leaven/workspace.materialize` → `kind: "call"`, `call_kind: "workspace_materialize"`.
- `leaven/proposal.submit_batch`, `leaven/proposal.apply`, `leaven/assessment.submit`, `leaven/evaluation.request`, `leaven/event.emit` → `kind: "write"`.
- `leaven/watch.{start,next,ack,cancel}` → engine MAY use `kind: "query"` for `start`/`next`/`ack` and `kind: "write"` for `cancel`, or define a watch-specific kind in a v1.x extension; this profile does not pin.

When a method's result contains redacted content (truncation, projection-narrowing, secret elision), `_meta.leaven_redactions` MUST be present and MUST itemize each redaction with `Redaction.reason` drawn from the closed reason set in `common.schema.json`.

When a method's behavior is a replay (returning cached/recorded values rather than executing freshly), the receipt's `kind: "call"` variant uses `status: "replayed"` or `status: "cached"`. Workers MUST treat these statuses as identical to `"succeeded"` for correctness purposes but MAY surface them differently in observability.

Fetching a receipt's full body (the `value`, `lm_response`, `agent_session`, etc. payload behind the ref) requires `leaven/plan_result.fetch`. The receipt returned inline in `_meta.leaven_receipt` is intentionally minimal: enough for the worker to cite, not the full value.

---

## 13. Error model

ACP errors follow JSON-RPC 2.0 (`docs/protocol/error.mdx`, `docs/protocol/overview.mdx:196-202`). Leaven layers a closed error code set on top, transported as `error.data.leaven_code` (string) and `error.data.leaven_details` (typed object) inside the standard JSON-RPC error envelope.

### 13.1 JSON-RPC envelope

The standard error response shape applies:

```
{
  "jsonrpc": "2.0",
  "id": <request id>,
  "error": {
    "code": <integer>,
    "message": <human string>,
    "data": {
      "leaven_code": <string from closed set>,
      "leaven_details": { <typed details> },
      "leaven_receipt": <ReceiptRef of the failure receipt, optional>
    }
  }
}
```

`code` (the integer) follows JSON-RPC conventions:

- `-32600`/`-32601`/`-32602`/`-32603`/`-32700` — reserved for JSON-RPC framing errors. The engine MUST use these for malformed envelopes, unknown methods, invalid params, internal errors, parse errors respectively.
- `-32000` through `-32099` — reserved for implementation-defined server errors. Leaven uses these for the closed `leaven_code` set; the integer mapping is informational and not normatively pinned (because callers MUST switch on `leaven_code`, not on the integer).

### 13.2 Closed leaven_code set (v1)

The v1 `PlanErrorV1.code` enum (closing the open string in the current schema, per `COMPREHENSIVE_DESIGN_PASS_NOTES.md` §5.4 issue 59) is:

- `token_invalid` — bearer not recognized; grant not in store.
- `token_expired` — grant's `expires_at` has passed.
- `capability_denied` — grant exists but does not authorize the requested action/resource/constraints.
- `budget_exceeded` — call would exceed the grant's `max_usd_micro`, `max_calls`, or aggregate token budget.
- `hidden_partition_violation` — query or assessment would expose a hidden partition not permitted by grant.
- `schema_validation_failed` — params did not match the relevant schema (`leaven.plan.v1`, `leaven.stage_payloads.v1`, etc.).
- `stage_runtime_error` — stage-side failure that is the worker's fault.
- `precondition_failed` — a write's `PreconditionV1` did not hold at commit time.
- `rate_limited` — concurrency or rate-limit cap was hit.
- `cancelled` — request was aborted by `session/cancel` or token revocation.
- `timeout` — request exceeded the grant's or method's timeout.
- `provider_policy_denied` — an LM/agent/sandbox provider policy refused (typically arrives via MCP-over-ACP but echoed when surfaced through this profile).
- `data_class_violation` — declared input classes violated the grant's allowed/forbidden sets.
- `quota_exceeded` — non-monetary quota (rows, bytes, materialized artifacts, watch backlog) was hit.

The engine MUST NOT use error codes outside this set in v1. Adding new codes requires a minor version bump and explicit additive update to this profile.

### 13.3 Failure receipts

A failed call that spent money (LM provider charge, agent runtime time) MUST still emit a receipt with `status: "failed"` and a `cost: Cost` that reports the actual charge. The receipt MUST be returned via `error.data.leaven_receipt`. This rule restates `leaven_public_seam_v1_lock_spec.md` §4.8.

### 13.4 Retryability

`error.data.leaven_details.retryable: boolean` indicates whether the worker MAY retry the same request. The engine MUST set `retryable = true` only for codes whose semantics tolerate retry (`rate_limited`, `timeout`, transient `provider_policy_denied` cases). `token_invalid`, `capability_denied`, `schema_validation_failed`, `hidden_partition_violation`, and `data_class_violation` MUST always be non-retryable.

---

## 14. Heartbeat and timeouts

ACP v1 does not define a native heartbeat. This profile layers one.

### 14.1 Heartbeat

For long-running stage calls (anything expected to run more than 60 seconds without producing a `session/update`), the worker SHOULD emit a heartbeat `session/update` notification at most every 30 seconds, carrying `_meta.leaven_heartbeat: { uptime_ms: <int>, queue_depth: <int> }`.

The engine MAY drive its own timeout countdown from heartbeat freshness. If no heartbeat or other `session/update` arrives within `2 * heartbeat_interval` (default 60 seconds), the engine MAY issue `session/cancel`.

The worker MAY decline to heartbeat; in that case the engine's only timeout signal is request-level. The engine SHOULD document its default request timeout to operators (default: 5 minutes for managed calls; 30 seconds for graph reads).

### 14.2 Per-method timeouts

Every `leaven/*` method respects the grant's `constraints.limits.timeout_s` when present. The engine MUST enforce the timeout server-side; the worker SHOULD also enforce a client-side timeout slightly longer (to allow engine-side cleanup before client-side error reporting). On timeout the engine MUST respond with `timeout` per §13.

### 14.3 session/cancel response time

When the engine issues `session/cancel`, the worker MUST acknowledge by either:

- Returning `cancelled` on any in-flight request within 5 seconds, or
- Closing the connection.

A worker that fails to acknowledge `session/cancel` within 10 seconds MAY be forcibly killed by the engine (e.g., `SIGKILL` on stdio workers). The engine SHOULD prefer graceful shutdown but MUST guarantee forward progress when workers misbehave.

---

## 15. Capability expiry mid-session

A capability grant's `expires_at` is enforced by the engine at every method invocation.

### 15.1 Graceful drain

When `expires_at` passes during an active session:

- In-flight `leaven/*` requests whose evaluation started before expiry MAY complete normally. The engine MUST NOT abort them mid-flight solely because of expiry.
- New requests received after expiry MUST be refused with `token_expired`.
- The engine MUST send a `session/update` notification carrying `_meta.leaven_token_expired: { expired_at: <Timestamp> }` so the worker can begin a graceful shutdown.
- The worker SHOULD finish any short cleanup and then close the connection or issue `logout`.

### 15.2 No mid-session token rotation

V1 does not provide token rotation. A worker that needs a longer-lived authority MUST request a fresh capability out-of-band (via the operator path) and start a new session on a new connection. Engine MAY refuse to spawn replacement workers if doing so would violate the parent grant's delegation depth or `expires_with_parent` constraint.

### 15.3 Long-running calls

A call expected to run beyond the grant's `expires_at` SHOULD be rejected at issue time (`expires_at - now < expected_duration`) rather than midway. The engine's per-method timeout (§14.2) and the grant's `expires_at` together bound call duration. Workers SHOULD plan stage calls so that managed effects do not straddle expiry.

---

## 16. Implementation requirements

This section summarizes the binding obligations on engine and worker shim implementations beyond what's already stated above.

### 16.1 Engine

- One authorization kernel for all transports. The engine MUST NOT short-circuit capability checks for stdio "trusted" connections; the bound grant is the authority regardless of transport.
- Bounded inbound queues for `session/update` notifications.
- Cancellation-respecting async: every `leaven/*` handler MUST poll for session cancellation cooperatively at each await point.
- Receipt threading on every method, including failure receipts on calls that spent money.
- Capability-fingerprint pinning on every response: `_meta.leaven_capability_fingerprint` is REQUIRED for audit trail completeness.
- OS-level execution policy enforcement for `managed_sandbox`, `package_scorer`, and `remote_untrusted` profiles. Capability tokens cannot prevent a process from opening a socket; the engine MUST pair them with container/namespace/seccomp constraints (per `leaven_public_seam_v1_lock_spec.md` §6.3, §6.4).
- Token-store hygiene: bearer secrets MUST be hashed at rest (HMAC or equivalent); plaintext-in-memory MUST be bounded by token lifetime.

### 16.2 Worker shim

- Bounded outbound mpsc for `session/update`, with overflow policy explicit at shim init.
- Token scrubbing from environment after `authenticate`.
- Cooperative cancellation: every long-running operation MUST poll for `session/cancel` and abort cleanly.
- Receipt forwarding: when a worker's host-language interior makes downstream decisions that depend on a managed call's outcome, the receipt SHOULD be carried alongside the value so the caller can cite it in subsequent submissions.
- Data-class declaration on every `leaven/*` call. Understatement is a contract violation.
- Heartbeat emission for stage calls expected to exceed 60 seconds.
- Graceful shutdown on `session/cancel`, `session/close`, capability expiry, or `stdin` EOF.

### 16.3 Both sides

- Strict ACP v1 protocol-version pin. No silent upgrade to v2.
- Frame size limit (default 16 MiB) enforced and configurable.
- Error codes drawn from §13.2 only.
- `_meta` discipline: only Leaven-registered `_meta` keys are interpreted; unknown keys MAY be preserved on forwarding but MUST NOT change behavior. Reserved `_meta` keys are `traceparent`, `tracestate`, `baggage` per `docs/protocol/extensibility.mdx:30-37` (W3C trace context); the engine SHOULD propagate these into trace receipts.

---

## 17. What this profile does NOT cover

Restated for emphasis:

- **LM, agent, sandbox, and human dispatch.** See the companion `leaven_mcp_over_acp_profile_v1.md`. Workers MUST NOT use this profile to make LM calls.
- **Pre-spawn worker discovery, registry, marketplace.** Operators wire workers in by hand or via deployment tooling outside the protocol.
- **MCP server federation.** This profile uses MCP-over-ACP for engine-to-worker sampling, not for connecting third-party MCP servers. Third-party MCP servers (if any) are connected by ACP's `session/new.mcpServers` mechanism, independent of this profile.
- **ACP v2 features.** Session fork, elicitation, custom LLM endpoints, HTTP transport, and the v2 schema bundle are out of scope. Implementers MAY track v2 development; this profile does not.
- **IDE-shaped UX.** Agent plans, slash commands, terminals, next-edit-suggestion. ACP supports these for IDE clients; Leaven engine clients ignore them.
- **Watch protocol semantics beyond the wire methods.** Delivery guarantees, backpressure granularity beyond the §8.2 rules, and lifetime semantics are owned by `leaven.watch.v1` (currently underdeveloped per `COMPREHENSIVE_DESIGN_PASS_NOTES.md` §5.8); this profile only specifies how the watch methods are dispatched over ACP.

---

## 18. Compatibility and versioning

### 18.1 Pinned to ACP v1

This profile is bound to ACP `protocolVersion: 1`. A future Leaven ACP profile v2 may target ACP v2; v1 implementations MUST NOT silently follow ACP v2.

### 18.2 Additive evolution

New `leaven/*` methods, new `_meta.leaven_*` fields, and new closed-enum values (extended cautiously) MAY be added in minor revisions of this profile (`leaven.acp.v1.1`, `leaven.acp.v1.2`, etc.). Workers MUST accept unknown `_meta` keys without error. Engines MUST refuse unknown `leaven/*` method names with JSON-RPC `Method not found` (`-32601`).

### 18.3 Breaking changes

Removing a `leaven/*` method, removing a closed-enum value, changing a method's capability action, or changing the receipt shape requires a major version bump (`leaven.acp.v2`). At a major bump, the `authMethods.id` MUST change (`leaven.capability.v2`) so misconfigured pairs fail at `authenticate` rather than at runtime.

### 18.4 Schema bundle compatibility

This profile rides on top of the v1 schema bundle (`leaven.plan.v1`, `leaven.plan_result.v1`, `leaven.capability.v1`, `leaven.stage_payloads.v1`, `leaven.evaluation_job.v1`, `leaven.evidence_envelope.v1`, `leaven.watch.v1`). Bumping a schema independently is permitted as long as the bumped schema's wire shape stays compatible; otherwise this profile MUST bump too.

### 18.5 Profile identifier

The profile identifier is `leaven.acp.v1`. Engines MUST advertise it in `InitializeResponse._meta.leaven_acp_profile_version = "1"`. Workers MUST pin to it in `InitializeRequest._meta.leaven_profile = "leaven.acp.v1"`.

---

## 19. Risks and watchitems

The non-exhaustive list of risks that could indicate this profile got something wrong; track these over the v1.x lifetime.

### 19.1 ACP version drift

ACP v2 has landed. Multiple RFDs are in flight. If v2 features become broadly adopted before this profile reaches stable use, Leaven users will pressure for v2 adoption. Treat the pressure carefully: do not adopt v2 features into v1 of this profile; cut a `leaven.acp.v2` when warranted.

### 19.2 Backpressure correctness

The bounded-mpsc requirement is the single most important integration-level rule. If worker shims slip back to unbounded queues (for ergonomic reasons, performance reasons, or copy-paste from the reference SDK), the OOM hazard returns. The first published worker shims for Python and TypeScript MUST be audited specifically for this rule.

### 19.3 Naming-inversion friction

ACP documentation calls the JSON-RPC sides "client" and "agent." Leaven's docs and code call them "engine" and "worker." Bugs in this surface will look like role confusion. Mitigate by: explicit naming in every error message, structured trace span attributes naming the Leaven role, and source-code comments at every `impl acp::Client` and `impl acp::Agent` boundary.

### 19.4 request_permission overload

Reinterpreting `session/request_permission` as a programmatic capability check is per ACP spec but stretches the original intent. If ACP's elicitation RFD (in flight) lands a cleaner primitive for programmatic capability checks, this profile SHOULD migrate when adopting it would not break v1 receipt audit trails.

### 19.5 MCP-over-ACP v1 stability

The companion `leaven_mcp_over_acp_profile_v1.md` rides on MCP carried over the ACP socket. The ACP MCP-over-ACP RFD is unstable as of May 2026; v1 may need to fall back to a separate MCP-server-process spawned by the engine to which the worker connects independently. The two profiles are co-designed but separately versioned; track stability separately.

### 19.6 `_meta` key collisions

The Leaven extension mechanism crowds the `_meta` namespace with `leaven_*` prefixed keys. If any other ACP user adopts the same prefix, key collisions are possible. Engines and workers MUST prefix all keys with `leaven_` and treat any other prefix as not-ours; future profile revisions MAY tighten this with a `_meta.leaven.*` nested object pattern, which is permitted under ACP's `_meta` shape (`{ [key: string]: unknown }`).

### 19.7 Long-running stage calls vs. grant expiry

Real evaluators take 20 minutes; capability grants are typically scoped to 20 minutes. The graceful-drain rule (§15.1) is correct but means stage calls that hit expiry mid-run will partial-fail in subtle ways. Operators SHOULD provision capability lifetimes with generous margin. If real workloads frequently hit expiry, v1.x may need an attenuated renewal flow.

### 19.8 Receipt volume on the wire

Every method returns a receipt. For evaluators producing thousands of operations across hundreds of cases, receipts can dominate the wire bytes (per `COMPREHENSIVE_DESIGN_PASS_NOTES.md` §11.6). If receipt volume becomes a real bottleneck, v1.x may add a `_meta.leaven_receipt_ref_only: true` flag letting the worker opt into receiving only the `ReceiptRef` and fetching the body on demand via `leaven/plan_result.fetch`.

### 19.9 Token bearer in the `authenticate` request

The bearer travels in-band exactly once, in the `authenticate` request's `_meta.leaven_capability_token`. The engine MUST scrub it from any post-authentication audit log entry. Worker-side, the bearer also reaches `stderr` if a misconfigured shim logs `_meta` payloads. This is a real footgun; worker shim authors MUST treat `_meta.leaven_capability_token` as a redaction-required field in any logging path.

---

## 20. Unresolved questions

These are flagged for resolution before v1 lock; they may become §19 watchitems if they survive to publication.

- **20.1** Whether `leaven/watch.next` deliveries via `session/update` notification should mirror `leaven/watch.next` request semantics exactly (same cursor advancement) or whether the push channel uses an independent acknowledgment cursor. Current spec uses the same cursor for simplicity; this may need a v1.x sharpening if real watch consumers want both pull and push concurrently.
- **20.2** Whether `session/load` for stage-call resume should replay receipts as `session/update` notifications. Spec currently allows but does not require. Implementations that omit replay risk hidden state divergence; implementations that include replay risk receipt-volume explosion on resume. Resolve before lock.
- **20.3** Whether `leaven/workspace.release` should be added in v1 for explicit handle release before session end. Current spec defers; adding it later is additive and safe.
- **20.4** Whether the JSON-RPC integer `code` ranges for closed `leaven_code` values should be normatively pinned. Current spec says informational; pinning enables clients that switch on the integer (faster code paths), at the cost of brittleness on `leaven_code` enum evolution.
- **20.5** Whether `_meta.leaven_capability_token` should be replaced by a separate non-`_meta` `params.capability_token` field on `authenticate`, to make the redaction-required nature visually obvious. Current spec uses `_meta` to keep the params shape unmodified vs ACP's `AuthenticateRequest`; alternative is cleaner but custom.

---

## 21. Profile self-description

Profile identifier: `leaven.acp.v1`.

Schema bundle dependencies: `leaven.plan.v1`, `leaven.plan_result.v1`, `leaven.capability.v1`, `leaven.stage_payloads.v1`, `leaven.evaluation_job.v1`, `leaven.evidence_envelope.v1`, `leaven.watch.v1` (where the wire methods reference its objects).

Transport substrate: Agent Client Protocol v1 over stdio (canonical), Unix domain socket, or HTTPS/WebSocket.

Replaces: the deleted `leaven.worker_protocol.v1.schema.json`.

Companion profile: `leaven_mcp_over_acp_profile_v1.md` (LM/agent/sandbox/human dispatch).

This document is normative for the wire and tone-matched to `leaven_public_seam_v1_lock_spec.md`. When this profile and the lock spec disagree on transport wire shape, this profile wins. When they disagree on schema content or semantics, the lock spec and the schema bundle win.
