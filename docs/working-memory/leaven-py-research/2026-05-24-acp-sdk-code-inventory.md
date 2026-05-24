# `leaven-acp` SDK code inventory and Path B (write-from-scratch) estimate

Status: working-memory note (planning input), not implemented proof
Created: 2026-05-24
Author: SDK inventory subagent

## Purpose

Validate or refute the in-conversation 5-7 day estimate for writing
`leaven-acp` from scratch instead of depending on `agent-client-protocol`.
Concretely: enumerate the modules that the in-house transport must own, line-
count them against the vendored SDK at
`/Users/darin/vendor/github.com/agentclientprotocol/rust-sdk/`, identify the
non-trivial pieces, and name the tests that will move the three blocked
conformance rows. The estimate must reflect that the locked Leaven ACP profile
is intentionally a narrow subset of the SDK (no proxy/conductor, no MCP-over-
ACP, no successor envelopes, no protocol-version negotiation).

The current state matters: `crates/leaven-acp` already exists as a blocking
stdio implementation in 327 LOC of `src/stdio.rs` plus 943 LOC of test
(`tests/stdio_session_contract.rs`). See AGENTS.md (lines 36-40) for the
explicit "do not fetch the SDK yet" rule. The numbers below treat this as a
v0 prototype; the Path B work is the async, supervised rewrite that can carry
live workers and unblock the matrix rows, not a green-field starting point.

## 1. The minimal `leaven-acp` surface

Topology is locked: `crates/leaven-acp` owns hot stdio process/session
transport for the locked public seam; everything semantic (method names,
Plan IR validation, extension-result envelopes, capability grants,
lifecycle vocabulary) stays in `leaven-public-seam`. See
`crates/leaven-acp/AGENTS.md` lines 1-28, `crates/AGENTS.md` lines
48-53, `crates/leaven-public-seam/AGENTS.md` lines 46-52.

```
src/lib.rs (~25 LOC) - map only; module declarations + curated re-exports.

src/error.rs (~120 LOC)
  Purpose: typed `AcpTransportError` enum, conversions from std::io,
           serde_json, and `PublicSeamError`. Already exists inline at
           crates/leaven-acp/src/stdio.rs lines 20-44 (~25 LOC); promote to
           its own module so the async runtime, framing, and dispatch can
           share the variants without circular module deps. Add timeout,
           cancellation, queue-full, and exit-status variants the blocking
           version glosses over.
  Owns: every fallible transport boundary.
  Does NOT own: any wire schema. PublicSeamError wraps and surfaces seam
           failures unchanged.

src/launch.rs (~150 LOC est)
  Purpose: project AcpStdioWorkerLaunch into a tokio::process::Command,
           construct working-dir/env, capture stderr to a bounded ring
           buffer, kill-on-drop guard.
  Reference: vendored SDK
           src/agent-client-protocol/src/acp_agent.rs (552 LOC; ~280 LOC
           load-bearing once command parsing, monitor_child, ChildGuard, and
           stderr collection are isolated). Leaven version does not need
           shell-words parsing, JSON-config schemas, or the McpServer
           transport multiplex; the launch object already comes from the
           seam-validated `AcpStdioWorkerLaunch`.
  Types: `AcpProcessCommand`, `AcpWorker`, `WorkerGuard`.
  Does NOT own: env var name truth (LEAVEN_CAPABILITY_TOKEN, etc.) - those
           come from `AcpStdioWorkerLaunch::worker_env()` in the seam.

src/framing.rs (~120 LOC est)
  Purpose: line-delimited JSON-RPC framing over tokio AsyncRead/AsyncWrite.
           Read newline-terminated UTF-8 lines into `serde_json::Value`,
           write `Value` + `\n` + flush. Reject empty/invalid lines, surface
           one Protocol error per malformed frame.
  Reference: vendored SDK
           src/agent-client-protocol/src/jsonrpc/transport_actor.rs (119
           LOC). Leaven version omits the Sink-of-String indirection and
           serialization-failure split, because the seam validates wire
           shapes before the framing layer ever sees them.
  Types: `JsonLineReader`, `JsonLineWriter`, `FramingError`.
  Does NOT own: id generation, request/response correlation, method
           dispatch.

src/wire.rs (~200 LOC est)
  Purpose: closed JSON-RPC envelope helpers - `Request { id, method,
           params }`, `ResponseEnvelope { id, result-or-error }`,
           `NotificationEnvelope { method, params }`. Map between
           `serde_json::Value` and these enums; nothing else. The seam owns
           the shape of `params` and `result`.
  Reference: vendored SDK
           src/agent-client-protocol/src/jsonrpc.rs lines 1306-1450
           (`OutgoingMessage`, `ResponsePayload`, `Handled`,
           `IntoHandled`). Strip everything role-related and successor-
           related; Leaven has one client role and one agent role with
           no proxy chain.
  Types: `WireRequest`, `WireResponse`, `WireNotification`, `RequestId`.
  Does NOT own: validated `AcpJsonRpcRequestDocument` /
           `AcpJsonRpcResponseDocument` - those are returned by the seam
           after we hand it `Value`. We never construct them directly.

src/dispatch.rs (~300 LOC est)
  Purpose: the central async loop. Owns id->oneshot map for outgoing
           requests, an incoming-message stream, and the handler table for
           inbound notifications (`session/update`) and inbound requests
           (`session/cancel` from the worker side; the engine-client never
           originates session/cancel as a request, only as a notification).
           Single tokio task; no spawn-per-message because the seam-bound
           lifecycle is small and ordering must be honored.
  Reference: vendored SDK
           src/agent-client-protocol/src/jsonrpc/incoming_actor.rs (411
           LOC) and src/jsonrpc/outgoing_actor.rs (237 LOC). Leaven version
           collapses these two actors into one because Leaven does not
           need protocol-version compatibility (`ProtocolCompat`), peer
           rewriting (RoleId/HasPeer), retry-on-no-handler, dynamic
           handler registration (single static handler table), or response
           ack channels (no dispatch-loop synchronization with handlers).
  Types: `Dispatcher`, `PendingRequest`, `InboundHandler`.
  Does NOT own: process supervision, framing bytes.

src/session.rs (~250 LOC est)
  Purpose: ties everything together. `AcpStdioSession::spawn(package,
           profile, command, launch_env) -> Session`. Carries the
           `AcpWorkerSession` (profile-derived lifecycle facts), the
           dispatcher handle, the worker guard, the bounded progress
           queue, and the cancellation handle. Public methods are
           `call_extension`, `cancel_with_error`, `next_session_update`,
           `wait_for_exit`. Replaces today's `AcpStdioProcessSession`
           (327 LOC) with an async version that does not hold blocking
           mutexes across await points.
  Reference: vendored SDK
           src/agent-client-protocol/src/session.rs (765 LOC). Leaven
           version is much smaller because it has no SessionBuilder
           generic state machine, no `ActiveSession<'responder, Link>`,
           no proxy session handoff, no `Blocking`/`NonBlocking` marker
           types, no MCP server attachment, and no `read_to_string`-shaped
           stop-reason loop. The bulk of SDK session.rs is type-state
           ergonomics for IDE clients, not lifecycle semantics.
  Types: `AcpStdioSession`, `AcpStdioCancellationHandle`,
           `SessionProgress`.
  Does NOT own: session-id semantics (Leaven sessions are 1:1 with the
           worker process for V1; multi-session-per-worker is not in the
           profile).
```

Production module bodies: ~1165 LOC, plus ~400 LOC test fixtures and
~600 LOC behavioral tests for the async path on top of the existing 943
LOC blocking suite. The SDK's `agent-client-protocol` crate is 13,510
LOC across src/; Leaven targets ~9% because most SDK code is
proxy/conductor/MCP/builder/type-state/peer-rewriting machinery Leaven
does not consume.

## 2. SDK modules consulted vs modules omitted

Inventoried via `find /Users/darin/vendor/github.com/agentclientprotocol/rust-sdk/src/agent-client-protocol/src -type f -name "*.rs"`.

### Adapted (load-bearing semantic; rewritten for Leaven idiom)

- `src/stdio.rs` (88 LOC). Adapted into `leaven-acp/src/launch.rs` +
  `framing.rs`. Drops the `blocking::Unblock` indirection on a hot path in
  favor of `tokio::process::Command` piped stdin/stdout, since the rest of
  the Leaven workspace is tokio-based. Drops the debug-callback
  inspect-stream because the seam's bearer-token redaction already lives
  on the launch side; we can add an opt-in line callback later if a
  tracing review wants it.
- `src/jsonrpc/transport_actor.rs` (119 LOC). Adapted into
  `leaven-acp/src/framing.rs`. Two actors collapse to two functions:
  `read_one_line(&mut R) -> WireResult<Value>` and
  `write_one_line(&mut W, &Value) -> WireResult<()>`. The actor wrapper
  buys us nothing once we collapse outgoing/incoming dispatch into one
  task.
- `src/jsonrpc/incoming_actor.rs` (411 LOC) + `outgoing_actor.rs` (237
  LOC). Adapted into `leaven-acp/src/dispatch.rs`. The shape is the same
  - id -> oneshot map, select! over (transport_in, outgoing_in, reply_in)
  - but the actor count drops from four streams down to three, and the
  retry/dynamic-handler/peer-rewrite branches go away.
- `src/acp_agent.rs` (552 LOC). The ChildGuard pattern (lines 198-211)
  and `monitor_child` (lines 217-241) are adapted into
  `leaven-acp/src/launch.rs`. Drops `from_args`/`from_str` (110 LOC of
  command-line parsing), drops `zed_claude_code`/`zed_codex`/
  `google_gemini` shortcuts, drops `MetaCapability`-based stderr debug
  multiplexing.

### Cited (read for reference, in-house code patterns its approach)

- `src/jsonrpc.rs` lines 245-1212 (the `Builder` + `connect_with`
  composition). Useful to understand how the four background actors
  (outgoing, incoming, task_actor, responder run) are joined under
  `futures::try_join!` (lines 1262-1283). Leaven version uses
  `tokio::select!` against one main loop plus a kill-on-drop child;
  there is no separate `task_actor` because the seam-bound call surface
  is `call_extension(method, params) -> result` rather than
  arbitrary-task spawning.
- `src/jsonrpc/handlers.rs` (519 LOC). Useful to see the `Handled::No
  { message, retry }` pattern and the type-driven dispatch via
  `JsonRpcRequest::matches_method` + `parse_message`. Leaven version
  does not need the chained handler abstraction because we have exactly
  two inbound message kinds: `session/update` notification and
  `session/cancel` notification (in the worker-to-engine direction this
  is rare; mostly engine-to-worker).
- `src/role.rs` (307 LOC) + `src/role/acp.rs` (334 LOC). Cited to
  confirm Leaven needs neither `RoleId` nor `HasPeer`. The profile
  pins exactly two roles (`engine_client`, `worker_agent`) by name as
  string constants on the seam side (`AcpWorkerSession`), so the type-
  level role machinery does not buy anything in V1.
- `src/jsonrpc/task_actor.rs` (61 LOC). Cited to understand
  `Task::new` and `process_stream_concurrently`. Leaven version does
  not expose `cx.spawn()` for handler closures; handlers are owned
  internally by the session, so this whole abstraction is dropped.
- `src/util.rs` (211 LOC) + `src/util/typed.rs` (921 LOC). Cited to
  understand `MatchDispatch`, `MatchDispatchFrom`, `json_cast`, and
  the error-construction helpers. Leaven uses `serde_json::Value`
  directly because every payload either flows verbatim into the seam
  validator or comes back as a validated `AcpJsonRpcResponseDocument`.
  Skipping these saves ~1100 LOC.
- `src/schema/client_to_agent/requests.rs` (55 LOC). Cited for the
  `AuthenticateRequest`/`AuthenticateResponse` method-name binding (line
  18). Leaven does not emit `authenticate` over the wire because the
  launch env carries the bearer token; the seam's
  `authenticate_acp_session` resolves it without a wire round-trip. If
  the profile evolves to require a wire authenticate, add ~30 LOC then.
- `examples/simple_agent.rs` (31 LOC) + `examples/yolo_one_shot_client.rs`
  (113 LOC). Confirm `connect_with`/`connect_to` shape; SDK Client usage
  is ~80 LOC of Builder glue. Leaven's equivalent is `AcpStdioSession::
  spawn` + `call_extension`, similarly thin.

### Omitted (not needed for Leaven V1)

- `src/jsonrpc/protocol_compat.rs` (749 LOC) - v1<->v2 message translation.
  Leaven pins one ACP version per release; no negotiation surface.
- `src/jsonrpc/dynamic_handler.rs` (55 LOC) plus the dynamic-handler
  branches in `incoming_actor.rs`. Leaven's call surface is static.
- `src/role/mcp.rs` (90 LOC) and all of `src/mcp_server/` (8 files,
  1543 LOC). MCP-over-ACP is explicitly not V1.
- `src/role/acp.rs` lines 150-334 (Proxy, Conductor, ProxySessionMessages,
  successor-message routing). No proxy chain in Leaven.
- `src/concepts/*` (8 files, 940 LOC). Cookbook prose, not behavior.
- `src/cookbook.rs` (12 LOC) + `src/component.rs` (255 LOC) - generic
  `ConnectTo<Counterpart>` trait powering proxy chains.
- `src/capabilities.rs` (223 LOC) - `MetaCapability` framework for
  `_meta.symposium`. The Leaven `CapabilityDocument` is a different
  concept; the name must not collide.
- `src/schema/proxy_protocol.rs` (207 LOC), `schema/v2_impls.rs` (424
  LOC), `schema/enum_impls.rs` (113 LOC), and the bulk of `schema/`
  except method-name constants. The seam owns actual schemas in
  `docs/specs/public-seam-v1/schemas/`.
- `src/handler.rs` (7 LOC) + `src/acp.rs` (9 LOC) + the macro hacks in
  `lib.rs` (`to_future_hack` lines 158-214). Required by SDK's
  `AsyncFnMut` design; not needed when handlers are owned internally.
- `src/typed.rs` (125 LOC) - `JsonRpcMessage`/`JsonRpcRequest`/
  `JsonRpcNotification` traits and derive. Seam returns validated
  documents; no derive needed here.
- `src/session.rs` (765 LOC) - SDK session is a SessionBuilder
  type-state machine for IDE ergonomics. Leaven needs only the
  lifecycle-state core; net save vs reuse ~515 LOC.

Cumulative omit: ~6,500 LOC out of 13,510, plus ~1,100 LOC of
`MatchDispatch`/`json_cast` helpers Leaven sidesteps. Roughly half the
SDK never appears in the Leaven adapter even conceptually.

## 3. The five hard pieces

These are the modules where the in-house implementation is non-trivial.
Listed in the order they should be implemented; each names the SDK file
read for reference.

### 3.1 Async request/response correlation (dispatch.rs, ~140 LOC of the 300)

SDK ref: `src/jsonrpc/incoming_actor.rs` lines 56-205 (the
`incoming_protocol_actor` select loop + `pending_replies:
HashMap<Value, PendingReply>`) and `outgoing_actor.rs` lines 27-127
(the `ReplyMessage::Subscribe` channel for registering pending awaits).

Design: collapse to one task. SDK has two actors because of
`ProtocolCompat`-driven response rewriting; Leaven does not. The Leaven
loop selects over `transport_lines` (Stream<io::Result<String>>),
`outgoing_requests` (mpsc<OutgoingRequest>), and `cancel_signal`
(tokio_util CancellationToken from the supervisor/handle).

On request submit: generate monotonic numeric id, register
`oneshot::Sender<WireResult<Value>>`, write the line. On response line:
look up id, complete oneshot. On notification (`session/update`,
`session/cancel`): route to lifecycle handler. Hard part: honor the
seam's bounded progress queue under backpressure without deadlocking
the engine call when the queue fills.

Blocking version (`src/stdio.rs` lines 206-216) sidesteps by reading
synchronously inside `call_extension`'s loop. Async cannot, because
`next_session_update` is a separate caller. Biggest async-vs-sync rework.

### 3.2 Cancellation propagation through async dispatch (~70 LOC)

SDK ref: no exact analog. SDK uses
`futures::future::select(protocol_future, child_monitor)` in
`acp_agent.rs` lines 339-353; per-request cancellation lives in
`SentRequest::on_receiving_result` callbacks. Leaven needs richer
behavior: the seam defines `AcpSessionCancellation` (receipt + closed
PlanError) that must reach the worker (as a `session/cancel`
notification) *and* every in-flight `call_extension` await (as
`AcpTransportError::Cancelled`).

Design: use `tokio_util::sync::CancellationToken`. On cancel: write
notification, set lifecycle Cancelled, fire token. The select loop sees
the token, drains pending-replies, completes each oneshot with
Cancelled, stops new outgoing requests. Tricky: the worker may still
write a response after we cancel - either consume-and-drop or drop the
read half cleanly. Test under slow-worker delays.

### 3.3 Bounded progress queue with backpressure policy (~80 LOC)

Seam reference:
`crates/leaven-public-seam/src/acp_profile/lifecycle.rs` lines 94-140
(`AcpSessionLifecycle::offer_progress`) defines the three policies:
`PauseWorker`, `DropNoncriticalUpdates`, and `Disconnect`. The seam
returns one of `AcpProgressDisposition::{Enqueued, DroppedNoncritical,
Disconnected}` after each update.

Design choice: the SDK has no analog because its `SessionMessage`
channel is `mpsc::unbounded` (see session.rs line 90: `let (update_tx,
update_rx) = mpsc::unbounded();`). Leaven must use a bounded channel
between the dispatcher and the `next_session_update` consumer, and the
policy decision must happen *before* enqueue. The right shape is
probably a `tokio::sync::Mutex<VecDeque<AcpSessionUpdate>>` plus a
`Notify` for wakeups, with the dispatcher calling
`session.offer_progress(message, priority)` from inside the dispatch
loop and translating `Disconnected` into a cancellation.

The blocking version (`src/stdio.rs` lines 286-349, `handle_session_update`)
already does this for the sync case. The async version must avoid
holding the lifecycle mutex across `.await` boundaries; the current
sync code does and tokio-mutex would deadlock. Either switch to
`std::sync::Mutex` for short critical sections or extract a
`LifecycleSnapshot` value type that can be observed without a lock.

### 3.4 Session state machine + cleanup ordering (~110 LOC)

Seam reference: `AcpSessionState` (Running / Cancelled) in
`crates/leaven-public-seam/src/acp_profile/lifecycle.rs` lines 192-198.
The seam tracks the state and refuses progress updates after
cancellation (lines 99-103). The transport must enforce the same on the
outgoing side: refuse `call_extension` after cancellation, refuse a
second `cancel_with_error` after the first.

Design choice: hold an `AcpSessionState` snapshot in the dispatcher
itself so the per-message decision does not touch the seam mutex. Drop
ordering matters: when `AcpStdioSession` is dropped while a call is in
flight, the order must be (1) cancel the cancellation token, (2) drop
the outgoing-request sender (so the dispatcher loop exits), (3) kill
the child process, (4) await the supervisor task so we do not leak it.
The blocking version's `Drop` (line 441) just kills the child, which is
fine for a synchronous API but races with the async dispatcher.

The SDK's `acp_agent.rs::ChildGuard` (lines 199-211) shows the kill-on-
drop pattern but does not address the in-flight-await problem. The
Leaven shape is closer to the BlockingHandle pattern used elsewhere in
the workspace.

### 3.5 Authenticate hook plumbing into `CapabilityRegistry` (~50 LOC)

Seam reference: `crates/leaven-public-seam/src/package.rs::PublicSeamPackage::authenticate_acp_session`,
called from the test fixture at
`crates/leaven-public-seam/tests/acp_profile.rs` lines 676-682. The
profile (`docs/specs/public-seam-v1/profiles/leaven_acp_profile_v1_v0.3.md`
lines 19-27) says authentication maps to `leaven.capability.v1`: the
engine spawns the worker with three env vars and the worker authenticates
by presenting them back through ACP `authenticate`.

Design choice: the SDK has a wire-level `AuthenticateRequest`/
`AuthenticateResponse` (see `schema/client_to_agent/requests.rs` line
18). Leaven's profile uses the same idea but resolves it through the
capability registry rather than emitting an `authenticate` JSON-RPC call
per session, because the env vars already carry the bearer token and
fingerprint. The transport's job is to bind the call sites: on session
spawn, the engine side calls `PublicSeamPackage::authenticate_acp_session
(profile, registry, AcpAuthenticateRequest::opaque(bearer, expiry, fp))`,
gets back an `AcpAuthenticatedSession`, and stores it inside the
`AcpStdioSession` for later permission decisions (`authorize_acp_permission`,
the seam's `authorize_permission` route). The transport must surface
authenticate failures as a typed error before the worker has consumed any
budget.

This is small (~50 LOC) but easy to get wrong because the right boundary
is unintuitive: the wire never sees an authenticate frame at all in V1.
Future profile revisions could promote it to a real wire call; the
transport should make that change a 30-line patch, not a redesign.

## 4. Test surface

Each of the three blocked rows requires the same kind of evidence: live
process round-trips that exercise the locked Leaven `leaven/*` extension
methods over actual stdin/stdout, with negatives that kill plausible fake
implementations.

What the wire-validation layer already proves (and `leaven-acp` must not
duplicate):

- `crates/leaven-public-seam/tests/acp_profile.rs::acp_jsonrpc_requests_and_responses_bind_plan_ir_and_extension_results`
- `crates/leaven-public-seam/tests/acp_profile.rs::acp_jsonrpc_rejects_in_process_or_cross_method_fakes`
- Bounded-queue / cancellation / extension-result envelope semantics
  prove via `AcpSessionLifecycle` and `AcpExtensionResultDocument`
  directly.

What `leaven-acp` already proves at sync-blocking level
(`crates/leaven-acp/tests/stdio_session_contract.rs`):

- `stdio_session_starts_worker_process_with_profile_roles_and_env`
- `stdio_session_rejects_private_mcp_or_bare_process_protocols`
- `stdio_session_carries_extension_result_envelopes_for_all_v1_method_families`
- `stdio_session_rejects_cross_method_and_semantic_result_fakes_from_worker_process`
- `stdio_session_binds_worker_progress_to_bounded_acp_update_queue`
- `stdio_session_cancellation_reaches_live_worker_and_stops_later_calls`

What the async Path B transport must add to unlock each row:

### `ps1.acp.transport_profile`

Matrix at `docs/specs/public-seam-v1/conformance-matrix.yaml` lines
1314-1361. `partial_contract_test_evidence` already cites eight
public-seam tests; `blocked_on` says: "A production ACP process/session
transport owner must execute JSON-RPC over stdio and prove the
engine-client/worker-agent lifecycle through the public route; the
current evidence is wire-contract validation only."

Tests to add in `leaven-acp/tests/`:

- `async_stdio_session_carries_concurrent_extension_calls_in_order` -
  prove the dispatcher correlates IDs when two callers race
  `call_extension` against one worker.
- `async_stdio_session_rejects_in_process_only_shortcut` - construct an
  `AcpStdioSession` whose "worker" is a tokio task in the same process
  that bypasses the framing layer; the seam validation must still pass
  but the transport must report the trace doesn't cross a process
  boundary (smoke-detect; this is the row's `fake_pass_rejected` fact).
- `async_stdio_session_rejects_authenticate_with_expired_or_unknown_bearer` -
  the wire test already proves this in-memory; add the live-process
  version that exits non-zero from authenticate failure.

### `ps1.acp.extension_results`

Matrix lines 1404-1456. `blocked_on`: "A production ACP process/session
transport owner must carry extension-result envelopes over the ACP route
for all V1 method families; the current evidence rejects bare payloads
at the wire-contract layer only."

The existing blocking suite's
`stdio_session_carries_extension_result_envelopes_for_all_v1_method_families`
already iterates the full V1 method set; the async port can reuse the
fixture functions (`extension_result_cases`, etc., lines 340-540 of the
existing test) verbatim. The additional async-specific tests are:

- `async_stdio_session_streams_extension_result_after_progress_burst` -
  worker emits 16 noncritical updates then the result; the bounded queue
  policy `pause_worker` must hold the worker until the consumer drains.
- `async_stdio_session_rejects_response_after_close` - worker writes a
  response *after* the engine has dropped the session; verify it does
  not surface a delayed Ok back to a torn-down caller.

### `ps1.acp.lifecycle_backpressure`

Matrix lines 1458-1502. `blocked_on`: "A production ACP process/session
transport owner must bind cancellation and bounded progress queues to
live worker activity; the current evidence proves profile-derived
lifecycle vocabulary only."

The blocking suite's `stdio_session_binds_worker_progress_to_bounded_acp_update_queue`
and `stdio_session_cancellation_reaches_live_worker_and_stops_later_calls`
prove the simple cases. Async-specific additions:

- `async_stdio_session_disconnect_policy_terminates_worker_session` -
  set `flow_control.backpressure` to `disconnect`, exceed the queue,
  prove the worker sees `session/cancel` and the in-flight
  `call_extension` returns the closed PlanError receipt.
- `async_stdio_session_cancellation_during_in_flight_call_completes_the_await` -
  cancel while a `call_extension` is pending; the future must resolve
  with `AcpTransportError::Cancelled` carrying the cancellation receipt,
  not hang.
- `async_stdio_session_drop_during_in_flight_call_does_not_leak_tasks` -
  drop the session while a call is pending; the tokio runtime must report
  no leaked tasks in `JoinSet::detach_all` equivalents.

Total new test count: ~9 async-specific tests. Plus a port of the
blocking-suite fixtures to the async API (~200 LOC of fixture rewrite,
most of which is mechanical `mut self -> &mut self` and `.unwrap() ->
.await.unwrap()`).

## 5. Honest timeline estimate

Single engineer with Claude assistance, working from the current state
(blocking implementation + 943 LOC test suite + locked seam contracts).

| Phase | Estimate | Notes |
| --- | --- | --- |
| Scaffold (Cargo.toml deps, error.rs split out, module layout) | 0.5 day | Add tokio, tokio-util, futures-util to dev deps. Update AGENTS.md routing. |
| Launch + framing modules (the easy mechanical parts) | 0.5 day | These are tokio-port jobs and the SDK pattern is clear. |
| Dispatch core (hard piece 3.1) | 1.0-1.5 days | Single-task select loop, pending-replies map, oneshot completion. Iteration is required to settle on the channel shapes. |
| Cancellation + state machine (hard pieces 3.2 + 3.4) | 1.0-1.5 days | Race-free shutdown is the bottleneck. Plan on at least one full revision after the first test pass. |
| Bounded progress queue (hard piece 3.3) | 0.5-1.0 day | The seam already owns the policy; this is wiring + the "do not hold mutex across await" rule. |
| Authenticate plumbing (hard piece 3.5) | 0.5 day | Small but invariant-heavy. |
| Test port + new async tests | 1.0-1.5 days | The fixture functions in the existing test are reusable; the new tests are the ones in section 4. |
| `just check` cleanup, coverage ratchet, AGENTS.md updates | 0.5 day | Mandatory before claiming completion. |

Sum: **5.5-7.5 days** of focused work.

Confidence interval: **medium**. The 5-day floor is plausible because
~40% of the work is mechanical port from the existing blocking version,
the seam owns most of the wire validation, and the SDK provides a
reference pattern for the actor/dispatch shape. The 7-day ceiling is
realistic because async cancellation race conditions and bounded-queue
backpressure tend to require one or two design iterations against real
worker fixtures.

This estimate **does not include**:

- Codex-app-server integration as a real worker (separate work in
  `leaven-agent-codex-app-server`).
- The first external-language worker fixture (the existing tests use
  bash; the `docs/plans/2026-05-24-public-seam-v1-acp-transport-route.md`
  open question 4 leaves this deliberately unanswered).
- Promoting any conformance row from blocked to proven. That requires
  the adversarial review wave called out in
  `docs/plans/2026-05-24-public-seam-v1-acp-transport-route.md` lines
  144-153.
- Future SDK migration if external dependency approval lands.

## 6. Risks that could break the estimate

1. **Bounded queue + tokio-mutex deadlock (likelihood: high, impact: +1 day).**
   `AcpSessionLifecycle` uses `&mut self` APIs not designed to cross
   `.await`. Holding the seam lifecycle mutex across an await in the
   dispatcher will deadlock under contention. Fix is either a local
   lifecycle snapshot reconciled at sync points or a `parking_lot::Mutex`
   touched only on the dispatch task. The blocking code already does the
   latter; in async this becomes a hazard.

2. **`session/cancel` ordering vs in-flight responses (medium, +0.5 day).**
   When the engine cancels and the worker has already written a response,
   that response may arrive before the worker reads the notification. The
   seam contract says cancellation yields a closed PlanError + receipt,
   not a value. The right behavior is "first-write-wins to the oneshot,
   but always complete the oneshot before drop."

3. **String-typed request IDs at the seam boundary (low, +0.25 day).**
   `AcpJsonRpcRequestDocument::id()` returns `&str`. Numeric request id
   generation on the tokio side needs a lossless and stable string
   projection.

4. **No SDK feature flag for "everything Leaven does not need" (certain,
   validates Path B).** Vendoring-and-pruning the SDK is not viable -
   the actor design assumes `RoleId`/`HasPeer`/`ProtocolCompat`. Path B
   (from scratch) is genuinely the right call once external-dependency
   approval is the gate.

5. **Conformance row promotion requires adversarial review, not just
   green tests (certain, +0.5-1 day calendar).** The plan note lists nine
   negative-proof scenarios. The async port must actively try to fail
   each one. Engineering time for the negatives is in the test estimate;
   the review wave is separate calendar.

6. **External-language worker fixture choice (medium, +0.5 day).**
   Existing tests use bash. A python or node worker proves the env-var
   contract better but adds CI environment complexity. The plan's open
   question 4 is unresolved; the wrong first choice forces fixture
   rewrites.

7. **Blocking suite is good but partial (high, +0.5 day).** Bash scripts
   echo single canned responses. The async path needs multi-message
   conversations (progress burst then result, cancel mid-conversation)
   that bash cannot easily produce. A small Rust test-worker binary in
   `leaven-acp/tests/fixtures/` is likely needed (~200 LOC).

## Conclusion for the spec writer

The 5-7 day estimate is **defensible** given the current starting point
(blocking implementation + seam contracts + test fixtures in place). It
is *not* defensible as "build from a blank `src/` directory"; that lands
closer to 10-12 days.

Realistic working number: **6-8 calendar days** for one engineer with
Claude assistance, of which 4-5 days are core implementation (async
dispatch, cancellation, backpressure), 1-2 days are test port + new
async tests, and 1 day is verification + AGENTS.md updates + adversarial
review prep.

The five hard pieces in section 3 are where the timeline lives or dies.
None is beyond a normal Rust async engineer, but each needs real fixtures
and at least one revision against live worker behavior. The locked
Leaven profile keeps the surface small enough that rebuilding from
scratch is a sensible alternative to depending on a 13,510-LOC SDK whose
primary value-adds (proxy chains, MCP, protocol-version negotiation) are
all outside Leaven V1.

## File references

Cited inline above. Quick map: SDK root
`/Users/darin/vendor/github.com/agentclientprotocol/rust-sdk/src/agent-client-protocol/`
(stdio.rs, jsonrpc.rs, jsonrpc/{incoming,outgoing,transport,task}_actor.rs,
jsonrpc/handlers.rs, jsonrpc/run.rs, acp_agent.rs, session.rs, role.rs,
role/acp.rs, schema/client_to_agent/requests.rs, examples/);
Leaven transport `crates/leaven-acp/{AGENTS.md, src/stdio.rs, tests/stdio_session_contract.rs}`;
Leaven seam `crates/leaven-public-seam/{AGENTS.md, src/acp_profile{,/lifecycle,/extension_result,/methods}.rs, tests/acp_profile.rs}`;
routing `crates/AGENTS.md` lines 48-53; spec + plan
`docs/specs/public-seam-v1/profiles/leaven_acp_profile_v1_v0.3.md`,
`docs/specs/public-seam-v1/conformance-matrix.yaml` lines 1314-1502,
`docs/plans/2026-05-24-public-seam-v1-acp-transport-route.md`.
