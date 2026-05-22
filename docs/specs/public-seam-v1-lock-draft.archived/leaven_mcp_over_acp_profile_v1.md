# Leaven Model Context Protocol over ACP Profile v1

Status: candidate normative profile, companion to `leaven_public_seam_v1_lock_spec.md`. This document is the wire profile for the Leaven engine's callback channel: how worker processes invoke Leaven-managed LM, agent, sandbox, and human-review effects through Model Context Protocol (MCP) tool calls, tunneled over the Agent Client Protocol (ACP) socket established by the companion `leaven_acp_profile_v1.md` (the ACP profile document is referenced by name; this profile assumes its identity and authentication surface).

Audience: engine implementers, SDK authors (Python, TypeScript, Rust), reviewers of the Leaven public seam, and frontier model reviewers performing line-by-line audit.

Scope: this profile pins (a) which MCP version is targeted, (b) how MCP is tunneled over ACP per the `mcp-over-acp` RFD, (c) the v1-fallback architecture that lets us ship before the RFD is stable upstream, (d) the four normative Leaven tools (`lm.complete`, `agent.run`, `sandbox.exec`, `human.review`), (e) the mapping from each MCP `tools/call` to the corresponding `leaven.plan.v1` Call node IR shape, (f) capability-token gating on every tool invocation, and (g) the receipt, redaction, replay, and budget-metering semantics surfaced through `_meta` on tool results.

Out of scope: the ACP profile itself (extension methods `leaven/graph.query`, `leaven/case.load`, etc.), the IR value language (`leaven.plan.v1`), the capability grant grammar (`leaven.capability.v1`), the evidence envelope (`leaven.evidence_envelope.v1`), watch protocol (`leaven.watch.v1`), the bearer-token issuance flow, and engine-to-engine federation. These belong to their respective specs.

This profile is normative. Words MUST, MUST NOT, SHOULD, SHOULD NOT, MAY are used per RFC 2119, matching the discipline of the lock spec.

---

## 0. The judgment this profile carries

0.1 Workers call LM, agent, sandbox, and human-review effects *through Leaven*; the engine owns provider dispatch, cache, budget, replay, telemetry, and policy. The worker is a consumer of these effects, never a direct provider client.

0.2 Every effect call rides MCP `tools/call`. MCP's `sampling/createMessage` is upstream-canonical for LM dispatch but, in this profile, LM dispatch is realized as a `tools/call` to a normative Leaven tool named `lm.complete` so that the four effect surfaces (`lm.complete`, `agent.run`, `sandbox.exec`, `human.review`) share one tool dispatch shape and one capability gate.

0.3 The engine acts as MCP *server*; the worker acts as MCP *client*. This inverts the typical "client = IDE, server = LLM-app" framing because the worker is the agent-shaped process while the engine owns the authorized provider surface.

0.4 Every MCP tool call is gated by the capability token that authenticated the ACP session. The grant document carried by that token MUST be evaluated against each tool's arguments before the engine dispatches to any provider. Denial returns a typed MCP error, never a silently truncated success.

0.5 Tool-call arguments are typed by reference to the corresponding `leaven.plan.v1` Call shape (`LmCompleteCallV1`, `AgentRunCallV1`, `SandboxExecCallV1`, `HumanReviewCallV1`). The engine MUST validate arguments against those schemas; tool results MUST carry receipts and redactions in `_meta` using the same vocabulary as the IR's plan-result receipts.

0.6 LM dispatch is mediated, cached, budgeted, replayable. Workers do not see provider API keys, do not select providers directly, and do not bypass cache by default. `cache_policy: bypass` is a privileged knob, gated by the LM grant.

0.7 Data-class labels travel on every tool call. Workers MUST declare `_meta.leaven_input_classes` for each call; the engine MUST validate them against the grant's `allowed_input_classes` and `forbidden_input_classes` before dispatch. Egress containment is a wire-level invariant, not a convention.

0.8 This profile is layered, not invented. ACP carries the session lifecycle and Leaven extension methods. MCP carries the callback-shaped tool surface. We do not invent a third channel.

---

## 1. Version pin and stability posture

### 1.1 MCP version

This profile targets the MCP specification revision `2025-11-25` (the dated revision under `modelcontextprotocol/modelcontextprotocol` at `docs/specification/2025-11-25/`). The engine MUST advertise this revision during MCP initialization and MUST NOT accept worker `protocolVersion` strings predating it without an explicit downgrade decision recorded at the engine level.

The engine MUST declare the following MCP capabilities at initialization:

- `tools` with `listChanged: false` (Leaven exposes a fixed v1 tool set; additive tools require a profile version bump).
- `logging: {}` (engine may stream effect telemetry as MCP log notifications).
- The engine MUST NOT declare `resources`, `prompts`, `completions`, or `sampling` capabilities. This profile uses only the `tools/*` surface.

The worker (MCP client) MUST declare the `tools` capability set it consumes. It MUST NOT declare `sampling` or `roots`; the engine's worker-as-MCP-client model treats those surfaces as absent for v1.

### 1.2 ACP version and MCP-over-ACP RFD pin

This profile targets ACP v1 as the canonical transport baseline. It tracks the `mcp-over-acp.mdx` RFD authored under `agentclientprotocol/agent-client-protocol/docs/rfds/mcp-over-acp.mdx` and the v2 Rust SDK module at `agentclientprotocol/agent-client-protocol/src/v2/mcp.rs`, both of which are marked **UNSTABLE** upstream at the time of profile authoring. The engine MUST NOT depend on the upstream-unstable surface as its only realization. Section 4 specifies a v1-fallback transport that does not require the RFD.

### 1.3 Stability posture

The profile pins, durable for v1:

- The four normative tool names (`lm.complete`, `agent.run`, `sandbox.exec`, `human.review`) and their argument-to-IR shape mapping (Section 7).
- The capability-action mapping (Section 6).
- The `_meta` envelope keys for input-class labeling, receipts, redactions, and replay (Section 9, 10, 11).
- The transport realization choices (Section 4) MAY evolve additively: a new realization (e.g., once the RFD stabilizes) is permitted; existing realizations MUST continue to validate against this profile for the v1 lifetime.

Removing a tool, renaming a tool, changing a tool's capability action, or changing the IR mapping of an existing tool requires a new major profile version.

---

## 2. Roles and naming

### 2.1 JSON-RPC role mapping

- **Engine** acts as the MCP **server**. It advertises the tool list, handles `tools/list` and `tools/call`, dispatches provider effects, and emits log notifications.
- **Worker** acts as the MCP **client**. It calls `tools/call` for each managed effect.

This inverts the everyday MCP framing (where the "agent" or "LLM-app" is the server's *consumer* and the IDE-style host is the client). In Leaven the worker is the agent-shaped process and the engine is the privileged surface. JSON-RPC roles map cleanly: the side answering `tools/list` is the server regardless of which side "looks more like" an agent. Documentation MUST call out the inversion to avoid reader confusion; implementations need not invent new role names.

### 2.2 Connection topology

Each worker process has exactly one ACP session active at a time (the session created by ACP `session/new` on behalf of one stage call). Over that session, exactly one MCP-over-ACP connection per worker MUST be established. Multiple stage calls in the same worker process require multiple ACP sessions and therefore multiple MCP connections; the engine MUST NOT multiplex Leaven tool calls from different sessions over one MCP connection.

Within one MCP connection, the worker MAY issue multiple concurrent `tools/call` requests subject to the grant's `max_concurrent` constraint per action.

---

## 3. Relationship to the companion ACP profile

The ACP profile (`leaven_acp_profile_v1.md`, sibling document) owns:

- Session lifecycle (`session/new`, `session/cancel`, `session/load`).
- Capability-token authentication via `authenticate`.
- Programmatic permission flow via `session/request_permission`.
- Leaven extension methods for graph reads, case reads, workspace reads/materialize, proposal/assessment writes, evaluation requests, watch start/cancel.
- The bounded-channel and backpressure rules for ACP notifications.

This profile (MCP-over-ACP) owns:

- The four effect-tool surfaces and their wire shape.
- Tool-call authorization against capability grants.
- Receipt and redaction envelope on tool results.
- Cache and replay behavior for tool calls.
- The two transport realizations (Section 4).

The two profiles MUST share one capability token per worker session. The engine MUST NOT accept an MCP connection that is not bound to a previously authenticated ACP session.

---

## 4. Transport realizations

The profile defines two realizations. An engine MUST implement at least the v1 fallback (Section 4.2). An engine MAY additionally implement the primary RFD realization (Section 4.1); when both are available, the engine MUST advertise both during initialization and let the worker pick by configuration.

### 4.1 Primary realization: MCP-over-ACP per the RFD

When ACP v2 with `mcpCapabilities.acp: true` is available on both sides, the engine MUST tunnel MCP frames over the established ACP socket using the methods defined in `agentclientprotocol/agent-client-protocol/docs/rfds/mcp-over-acp.mdx` and the Rust types in `agentclientprotocol/agent-client-protocol/src/v2/mcp.rs`:

- `mcp/connect` (worker -> engine) opens an MCP-over-ACP connection identified by an ACP-side `acpId`. Per `mcp-over-acp.mdx` lines 162-191 and `src/v2/mcp.rs` lines 42-73, the connect request carries the `acpId` chosen by the side that declared the MCP server (here, the engine, declared via `session/new`).
- The engine MUST return a fresh `connectionId` (`src/v2/mcp.rs` lines 85-95) scoped to the ACP session.
- `mcp/message` (bidirectional) carries the inner MCP request/response/notification on the established `connectionId` (`src/v2/mcp.rs` lines 128-180 and `mcp-over-acp.mdx` lines 193-226).
- `mcp/disconnect` ends the MCP-over-ACP connection at end-of-session or on explicit teardown (`src/v2/mcp.rs` lines 279-310).

The engine MUST declare the Leaven MCP server in the `session/new` request's `tools.mcpServers` list with `"type": "acp"` and a Leaven-generated UUID `id` per `mcp-over-acp.mdx` lines 36-53. The worker MUST connect to that server when it needs effect dispatch.

This realization shares one process, one socket, and one credential context. It is the preferred realization for any engine deployment where both sides advertise ACP v2 with MCP-over-ACP support.

Because the RFD and v2 module are marked UNSTABLE upstream, engines using this realization MUST track upstream changes and MUST NOT treat the wire shape as locked beyond what this profile pins. If the upstream RFD changes incompatibly, the engine SHOULD downgrade to Section 4.2 rather than ship a divergent dialect.

### 4.2 Fallback realization: separate MCP server process per worker session

When ACP v2 with MCP-over-ACP is unavailable, the engine MUST run a dedicated MCP server process (the **Leaven MCP shim**, conventionally invoked as `leaven-mcp-server`) and expose it to the worker over a second transport channel. The engine MUST use exactly one of the following transports for the shim, chosen at session establishment and recorded in the ACP session metadata:

- **stdio**: a second stdio pair handed to the worker at spawn time (e.g., via inherited file descriptors `LEAVEN_MCP_IN`, `LEAVEN_MCP_OUT`, or an `env`-passed pipe path).
- **Unix domain socket**: a path under a per-session runtime directory passed to the worker via env (e.g., `LEAVEN_MCP_SOCKET=/run/leaven/<session>/mcp.sock`). The engine MUST set 0600 permissions and MUST tie the socket directory to the OS user running the worker.

The shim MUST speak the MCP `2025-11-25` stdio framing as defined in `docs/specification/2025-11-25/basic/transports.mdx` (UTF-8 JSON-RPC, newline-delimited for stdio; the same JSON-RPC framing inside Unix-socket bytes).

The shim is engine-owned. It MUST authenticate the worker on its first message by validating an out-of-band capability-token reference (see Section 5.3). The shim MUST NOT accept connections from any other process; it is a per-session, per-worker isolated surface.

Lifetime invariants for the fallback:

- The shim process MUST be spawned by the engine no earlier than ACP session establishment and MUST be ready before the engine returns `session/new` to the worker.
- The shim MUST refuse all `tools/call` requests until it has been pre-loaded with the worker's capability grant document by the engine.
- The shim MUST terminate when the ACP session closes (cleanly on `session/cancel`, on ACP socket EOF, or on capability-token expiry). The engine is responsible for SIGTERM and timeout-bounded SIGKILL.
- The shim and the engine communicate over an engine-internal channel that is out of scope for this profile (e.g., gRPC, Unix socket, in-process Rust API). The wire between worker and shim is what this profile pins.

The fallback is normatively complete: a v1 engine implementing only Section 4.2 is a conformant engine.

### 4.3 Realization equivalence

The four tool definitions, the capability-action mapping, the `_meta` envelope, and the receipt semantics in this profile are identical across both realizations. A worker SDK SHOULD be written against the abstract MCP `tools/call` surface and SHOULD treat the choice of realization as a transport configuration knob.

---

## 5. Authentication and authorization

### 5.1 Worker authenticates the ACP session

Per the companion ACP profile, the worker authenticates by calling ACP `authenticate` with the bearer capability token. The engine resolves the token to a persisted grant document by capability fingerprint and binds the resolved grants to the session. This profile assumes that has happened; it does not re-issue tokens at the MCP layer.

### 5.2 Same grants gate MCP tool calls

The grants on the ACP-authenticated token authorize MCP tool calls. The engine MUST evaluate every `tools/call` request against the grant matching the tool's capability action (see Section 6 for the mapping) using the algorithm in `leaven_public_seam_v1_lock_spec.md` §6.6. Specifically:

1. The token MUST contain a grant whose `action` equals the tool's required action (e.g., `lm.complete` for the `lm.complete` tool).
2. The grant's `resource` MUST permit the tool's requested resource (e.g., `lm_pool`, `runtime_pool`, `sandbox_pool`, `queues`).
3. The grant's `constraints` MUST permit the call's arguments: model in `models`, runtime in `runtimes`, argv prefix in `allowed_commands`, queue in `queues`, model role in `model_roles`, purpose in `purpose`/`purposes`, etc.
4. The call's declared `_meta.leaven_input_classes` MUST be a subset of `constraints.allowed_input_classes` and MUST be disjoint from `constraints.forbidden_input_classes`.
5. The call's projected USD cost (estimated or actual) MUST NOT exceed `constraints.limits.max_usd_micro` for the grant; the cumulative across this grant's lifetime MUST NOT exceed it either. The engine MUST refuse a call that would breach the ceiling.
6. The grant's `limits.max_concurrent` MUST be enforced across in-flight calls under this grant.
7. The grant's `limits.max_calls` MUST be enforced cumulatively over the grant lifetime.

Authorization is checked before any provider-side dispatch and before any side effect.

### 5.3 Fallback-mode token binding

For the Section 4.2 fallback, the worker does not authenticate against the MCP shim with a fresh credential. Instead, the engine MUST pre-register the resolved capability grants for that worker session with the shim out-of-band before the worker connects. The shim MUST refuse all calls received before that registration completes.

The worker MAY pass a short-lived correlation handle (e.g., `LEAVEN_MCP_SESSION=<opaque>`) in env so the shim can route the connection to the right session, but the handle MUST NOT be treated as a credential by itself. The credential remains the ACP-validated capability token; the shim trusts the engine's out-of-band registration.

### 5.4 Denial responses

When authorization fails, the engine MUST return a JSON-RPC error response on the `tools/call` (a *protocol error* per MCP `tools.mdx` lines 453-471), not a `tools/call` result with `isError: true`. The error code MUST be one of the codes pinned in Section 12. The error message SHOULD be short and non-leaking; structured leak-reason data MUST live in `data._meta.leaven_redaction` as a `Redaction` object (per `common.schema.json` `Redaction`).

The engine MUST NOT silently substitute a redacted payload for a denied call. The worker is entitled to see that a call was refused so it can take an alternative path (escalate to a human, drop the case, retry with narrower input classes).

---

## 6. Capability-action mapping

| MCP tool name   | Capability action   | Grant definition (`leaven.capability.v1`) |
| --------------- | ------------------- | ----------------------------------------- |
| `lm.complete`   | `lm.complete`       | `LmCompleteGrant` (schema lines 778-852)  |
| `agent.run`     | `agent.run`         | `AgentRunGrant` (schema lines 853-913)    |
| `sandbox.exec`  | `sandbox.exec`      | `SandboxExecGrant` (schema lines 721-777) |
| `human.review`  | `human.review`      | `HumanReviewGrant` (schema lines 914-951) |

The engine MUST NOT expose Leaven tools whose capability action is not in this table at v1. Tools for graph/case/workspace reads, proposal submission, assessment submission, evaluation requests, and watch lifecycle are served by the ACP extension methods listed in the companion ACP profile, not by MCP. The boundary is: callback-shaped effects -> MCP tools; graph-shaped reads and writes -> ACP extension methods.

---

## 7. The tool surface

Each tool subsection specifies: the MCP tool definition (name, title, description, `inputSchema` discipline, `outputSchema` discipline), the mapping from `inputSchema` to the corresponding `leaven.plan.v1` Call shape, the result mapping to the corresponding `leaven.plan_result.v1` value, and the cache/budget/replay semantics. The `inputSchema` and `outputSchema` documented here are normative; engines MUST publish them on `tools/list` exactly as specified (with the same field names, types, and discriminants), referencing the canonical JSON Schemas in `schemas/leaven.plan.v1.schema.json` for the wire shape.

### 7.1 `lm.complete`

- **Name**: `lm.complete`
- **Title**: "Leaven managed LM completion"
- **Description**: "Dispatch a language-model completion through the Leaven engine. The engine routes the request via the configured provider pool, applies cache/budget/replay/policy, and returns a parsed response with effect receipt."
- **Capability action**: `lm.complete`.
- **Input schema**: mirrors `LmCompleteCallV1` (`leaven.plan.v1.schema.json` lines 1703-1756). The `inputSchema` MUST require `purpose` and `messages`, MUST accept `model`, `model_role`, `output` (OutputContract), `sampling` (provider-neutral key-value), `cache_policy` (`default | bypass | require_cached | record_only`), `limits` (`CallLimitsV1`, lines 1815-1840), `input_classes` (`DataClassSet`), and `provider_hints` (provider-neutral key-value). The `messages` items MUST conform to `LmMessageV1` (lines 1757-1795).
- **Output schema**: `outputSchema` is required and MUST be the `lm_response` value shape from `leaven.plan_result.v1.schema.json` lines 155-192: required `kind: "lm_response"`, `receipt: ReceiptRef`, plus optional `cache`, `model_fingerprint`, `cost`, `message` (parsed message content), `parsed` (parsed structured output when `output.kind == "json_schema"`), `trace_ref`, and `redactions`.
- **Result `structuredContent`**: the engine MUST return the `lm_response` value as `structuredContent`. The engine MUST also serialize the same value into a `content[0]` text block for MCP backwards compatibility per `tools.mdx` lines 322-326.
- **`_meta` on result**: MUST carry `leaven_receipt` (the `ReceiptRef`), `leaven_redactions` (array of `Redaction` per `common.schema.json`), `leaven_replay_class` (one of `pure_read | fully_managed | boundary_managed | has_declared_external_effects | has_untracked_external_effects` per lock spec §15; for `lm.complete` this is always `fully_managed` on a live dispatch and `fully_managed` on cache hit or replay).
- **Cache**: governed by the call's `cache_policy`. Default cache key is content-fingerprint of (`purpose`, `model_or_role_resolved`, canonicalized `messages`, canonicalized `output`, canonicalized `sampling`, normalized `provider_hints`). `bypass` skips lookup. `require_cached` denies the call if no cache entry exists. `record_only` populates the cache without consulting it. The cache implementation is the `leaven-lm-cache` subsystem; this profile does not re-specify it.
- **Budget**: the call's projected cost MUST be checked against `LmCompleteGrant.constraints.limits.max_usd_micro` (and the worker's aggregate budget once §4.5 of the design notes is resolved at lock-spec level). The actual cost MUST be reported in `result.cost` and `_meta.leaven_receipt.cost`.
- **Replay**: under `EvalMode.replay { receipts }` (plan.v1 mode), if a receipt matches the call's `request_hash`, the engine MUST serve the recorded `lm_response` and MUST mark `result.cache: "replayed"`. The engine MUST NOT dispatch to a provider when replaying.
- **Provider policy**: the engine MAY redact provider output (e.g., remove provider-side moderation rationales) or refuse a call entirely (e.g., safety filter); refusals surface as a JSON-RPC error with `code: leaven.provider_policy_denied`; redactions surface in `_meta.leaven_redactions` and are not in-band content.
- **Mapping to IR**: the tool-call arguments, taken verbatim, instantiate `LmCompleteCallV1`. The engine MUST validate against the schema before dispatch.

### 7.2 `agent.run`

- **Name**: `agent.run`
- **Title**: "Leaven managed agent session"
- **Description**: "Run a provider-neutral agent session in a Leaven-managed workspace. The engine spawns the configured runtime under the agent runtime pool, supervises the session, enforces tool/network policy, and returns the session result with receipt."
- **Capability action**: `agent.run`.
- **Input schema**: mirrors `AgentRunCallV1` (lines 1841-1884). The `inputSchema` MUST require `runtime`, `workspace`, `instructions`, `output`, and MUST accept `env`, `tool_policy`, `limits`, `input_classes`. The `instructions` MUST conform to `AgentInstructionsV1` (lines 1885-1899). The `workspace` MUST be a `WorkspaceRef` previously returned by a `leaven/workspace.materialize` ACP call or by a prior tool call that returned a workspace handle; the worker MUST NOT fabricate `WorkspaceRef` values.
- **Output schema**: required, mirrors `agent_session` value shape (lines 193-232): `kind: "agent_session"`, `receipt`, optional `runtime_fingerprint`, `status` (`succeeded | failed | timeout | cancelled`), `parsed`, `output_files`, `cost`, `trace_ref`.
- **Result `structuredContent`**: the engine MUST return the `agent_session` value as `structuredContent`, and serialize it into `content[0]` text for compatibility.
- **`_meta` on result**: MUST carry `leaven_receipt`, `leaven_redactions`, `leaven_replay_class` (`fully_managed` when the agent ran under the managed runtime pool; degraded only with explicit operator override).
- **Cache**: agent sessions are not cached by content-fingerprint by default; the result is non-deterministic. `cache_policy` MAY be exposed as an additive extension under `_meta.leaven_cache_policy` for runtimes that support session replay (e.g., Codex app-server's transcript replay), but the v1 default is no-cache.
- **Budget**: agent sessions can run minutes-to-hours and accumulate cost incrementally. The engine MUST stream cost increments via MCP `notifications/message` log notifications on the same connection while the session runs, and MUST enforce `limits.max_usd_micro` by cancelling the session when the cumulative spend hits the cap. Cancellation surfaces as `status: "cancelled"` with the partial transcript still preserved in the trace.
- **Replay**: under `EvalMode.replay`, a matching receipt MUST yield the recorded `agent_session` (the engine MUST NOT respawn the runtime). For runtimes whose transcripts are too large to inline, `parsed` MAY be summarized and the full transcript fetched via the trace ref.
- **Tool policy**: `tool_policy` is grant-constrained; the engine MUST refuse runtimes outside `AgentRunGrant.constraints.runtimes`, MUST refuse tools outside `allowed_tools`, and MUST enforce `allow_shell` even if the runtime would otherwise expose a shell tool by default.

### 7.3 `sandbox.exec`

- **Name**: `sandbox.exec`
- **Title**: "Leaven managed sandbox command execution"
- **Description**: "Execute argv in a Leaven-managed sandbox or workspace under sandbox policy. Returns exit status, stdout/stderr refs, file output refs, and effect receipt."
- **Capability action**: `sandbox.exec`.
- **Input schema**: mirrors `SandboxExecCallV1` (lines 1900-1943). MUST require `workspace`, `argv` (non-empty array of strings), `output_contract`. MUST accept `env`, `stdin`, `timeout_s`, `input_classes`.
- **Output schema**: required, mirrors `sandbox_exec` value shape (lines 233-276): `kind: "sandbox_exec"`, `receipt`, `status`, optional `exit_code`, `stdout_ref`, `stderr_ref`, `files`, `cost`, `trace_ref`.
- **`_meta` on result**: MUST carry `leaven_receipt`, `leaven_redactions`, `leaven_replay_class` (`fully_managed`).
- **Cache**: by default, sandbox exec is not content-fingerprint-cached at v1. The exception is deterministic verifier-style invocations (e.g., a pinned linter, a hash-stable test runner) where the call MAY declare `cache_policy: default` via an additive `_meta.leaven_cache_policy`; this is opt-in and not required for v1 compliance. Replay-by-receipt remains canonical.
- **Budget**: sandbox compute is usually flat-rate or near-zero per call; the engine MUST still account it under `SandboxExecGrant.constraints.limits.max_usd_micro` when the sandbox pool charges.
- **Network**: enforced from the grant's `network: deny | allow | allowlist`. The engine MUST set the sandbox network policy from the grant before exec; the worker cannot request a different policy in the call arguments.
- **Replay**: receipt-keyed; replay MUST serve the recorded stdout/stderr refs and files map without re-execution.

### 7.4 `human.review`

- **Name**: `human.review`
- **Title**: "Leaven managed human review request"
- **Description**: "Submit a prompt to a Leaven-managed human review queue and await a verdict. Returns the verdict plus cost and receipt."
- **Capability action**: `human.review`.
- **Input schema**: mirrors `HumanReviewCallV1` (lines 1979-2011). MUST require `queue`, `prompt` (a `ContentTemplateV1`), `output`. MUST accept `rubric`, `limits`, `input_classes`.
- **Output schema**: required. v1 reuses the generic write-receipt shape extended with a `human_review` discriminant when the lock spec finalizes `human_review` as its own value kind in `leaven.plan_result.v1`; until then, the engine MUST return a value with `kind: "human_review"` matching the eventual addition under the same envelope semantics (receipt, cost, verdict, trace_ref, redactions).
- **`_meta` on result**: MUST carry `leaven_receipt`, `leaven_redactions`, `leaven_replay_class` (`fully_managed`). The receipt's cost MUST report `Cost.human_review_usd_micro` distinctly from `usd_micro` per `common.schema.json` design.
- **Asynchrony**: human review is asynchronous and may take hours. The engine MUST either (a) hold the JSON-RPC call open with periodic `notifications/message` log notifications until the verdict is ready, or (b) reject with a typed `leaven.async_pending` error and require the worker to poll via the watch protocol (engine's choice, surfaced at session establishment via `_meta.leaven_human_review_mode`).
- **Cache**: never cached.
- **Replay**: receipt-keyed; replay MUST serve the recorded verdict.

### 7.5 Forbidden tools

The engine MUST NOT expose any of the following over MCP at v1, even under additive extension:

- Graph reads (use ACP extension methods).
- Case reads (use ACP extension methods).
- Workspace reads or workspace materialize (use ACP extension methods).
- Proposal or assessment writes (use ACP extension methods).
- Evaluation requests, run-event emission, watch lifecycle (use ACP extension methods).
- Token issuance, delegation, or revocation (out of scope for this profile entirely).

A worker that encounters an engine exposing forbidden tool names MUST reject the session as non-conformant.

---

## 8. Sampling and the `lm.complete` mapping

MCP `sampling/createMessage` (`docs/specification/2025-11-25/client/sampling.mdx`) is the upstream-canonical mechanism for "server requests an LM completion through the client." This profile does *not* use `sampling/createMessage` for Leaven LM dispatch, for three reasons:

1. **Role inversion mismatch.** `sampling/createMessage` is server-initiated (server asks client). In our inversion the worker (acting as MCP client) is the one wanting LM dispatch from the engine (acting as MCP server). The arrow points the wrong way; using `sampling/createMessage` would require the engine to push and the worker to answer, which inverts the entire reason workers exist.

2. **Capability symmetry.** All four Leaven managed effects (LM, agent, sandbox, human) share the same gate, the same receipt shape, and the same `_meta` envelope. Forcing `lm.complete` onto `sampling/createMessage` would create a special case at the wire level that doesn't earn its keep.

3. **Leaven-specific extension fields.** `LmCompleteCallV1` carries `purpose`, `model_role`, `cache_policy`, `input_classes`, `provider_hints`, and `limits.max_input_tokens`/`max_output_tokens` that have no direct cognate in `sampling/createMessage`. We would thread all of them through `_meta` regardless. A normative `tools/call` definition is clearer than a non-trivial `_meta` overlay on a method designed for a different conversation.

The `lm.complete` `inputSchema` IS, however, designed to be a strict superset of the `sampling/createMessage` request semantics:

- `messages` may be lowered from `sampling/createMessage.messages` directly (the `LmMessageV1.role` enum is a superset of MCP's `user|assistant` because Leaven also distinguishes `system`, `tool`, `developer`; the engine MUST accept v1's role enum and may downconvert to provider-specific role conventions).
- `sampling/createMessage.modelPreferences.hints[].name` maps onto `LmCompleteCallV1.model` (single hint) or `LmCompleteCallV1.model_role` (when the grant exposes model roles rather than concrete model names).
- `sampling/createMessage.systemPrompt` maps onto a `messages[0]` with `role: "system"`.
- `sampling/createMessage.maxTokens` maps onto `limits.max_output_tokens`.
- Tool use within sampling (the `tools` array per `sampling.mdx` lines 145-262) is **not** wired through `lm.complete` at v1. Tool calls during LM completion are the LM's own tool calls, internal to the provider; the four Leaven managed effects are not LM-driven tool calls. If a Leaven adapter wants LM-driven tool calling (e.g., a function-calling proposer), it MUST be expressed as repeated `lm.complete` invocations from the worker, with tool execution loops owned by the worker. This is the same discipline §4.10 of the design notes flags as an open question for `LmCompleteCallV1` itself: v1 ships text + JSON-schema output; multimodal and in-LM tool calling are deferred.

When a future profile wants to bridge a Leaven worker with an external MCP server that itself uses `sampling/createMessage`, the bridge can map `sampling/createMessage` to `lm.complete` 1:1 at the boundary. This profile does not require that bridge.

---

## 9. Data-class labels on tool calls

Every `tools/call` request MUST carry `_meta.leaven_input_classes` as an array of `DataClass` values (the closed enum defined in `common.schema.json`, plus the `x.*` extension namespace). The labels describe what data classes flow *into* the call. The engine MUST evaluate them against the grant per Section 5.2 before dispatch.

When the IR-shaped `input_classes` field is also present on the tool arguments (as `LmCompleteCallV1.input_classes` etc.), the two MUST be equal as sets. If they disagree, the engine MUST refuse the call with a `leaven.input_classes_inconsistent` error rather than silently choosing one. This redundancy is intentional: tools defined by JSON Schema may pass `inputSchema` validation without carrying `input_classes` (since IR-side `input_classes` is optional), so the `_meta` field is the wire-level mandatory channel.

The engine MUST propagate input-class labels into the call's receipt and into any value derived from the call's result, per lock spec §6.7. This profile does not redefine the propagation rules; it only pins the wire convention.

---

## 10. Receipts on tool results

Every successful `tools/call` result MUST include `_meta.leaven_receipt` whose value is a `ReceiptRef` per `common.schema.json` (one of the typed-prefix receipt IDs: `lmrec_`, `agentrec_`, `execrec_`, `humanrec_`, `chargerec_`, plus the umbrella `effect_` prefix). The receipt MUST resolve via subsequent ACP queries (e.g., `leaven/graph.query` with a receipt-pointed source) into the full `OperationReceiptV1` shape defined in `leaven.plan_result.v1.schema.json` lines 390-439.

The receipt envelope MUST also include:

- `_meta.leaven_redactions`: an array of `Redaction` objects (`common.schema.json#Redaction`) with typed reasons. Empty array is permitted; absent field is forbidden when `_meta.leaven_receipt` is present.
- `_meta.leaven_replay_class`: the replay class for this call.
- `_meta.leaven_cache`: one of `hit | miss | replayed | bypassed`, mirroring the `lm_response.cache` enum. For `agent.run`, `sandbox.exec`, and `human.review` at v1, the value MUST be `miss` on live dispatch or `replayed` on receipt-replay.
- `_meta.leaven_call_kind`: one of `lm_complete | agent_run | sandbox_exec | human_review`. This is redundant with the tool name but enables receipt-correlation pipelines without parsing tool names.

The engine MUST NOT emit a successful tool result without these `_meta` fields. A worker that receives a result without them MUST treat it as a non-conformant engine response.

---

## 11. Cache, budget, replay behavior

### 11.1 Cache

`lm.complete` cache behavior is owned by `leaven-lm-cache`. The cache key derivation is implementation-defined for v1 but MUST be content-fingerprint-stable across engine restarts. Cache hits MUST report `_meta.leaven_cache: "hit"` and MUST still charge zero provider USD (the receipt's `cost.usd_micro` MUST be zero on a hit). Cache-related cost (e.g., embedding lookup) MAY be reported under `cost` extension fields.

The four `cache_policy` values mean exactly what they say:

- `default`: lookup; on hit serve; on miss dispatch and record.
- `bypass`: skip lookup; dispatch; record (unless `record_only: false` is implied by `bypass`).
- `require_cached`: lookup; on hit serve; on miss return a `leaven.cache_required_miss` error.
- `record_only`: skip lookup; dispatch; record. The same as `bypass` semantically; the distinction is intent metadata for downstream telemetry.

Engines MUST honor exactly the four values; additional values are forbidden at v1.

### 11.2 Budget

Per Section 5.2, every grant carries `limits.max_usd_micro` and `limits.max_calls`. The engine MUST refuse a tool call when:

- Projected cost + cumulative cost in this grant's lifetime > `limits.max_usd_micro`.
- Cumulative call count + 1 > `limits.max_calls`.
- Concurrent in-flight count + 1 > `limits.max_concurrent`.

The refusal MUST be a JSON-RPC error with code `leaven.budget_exceeded`, `leaven.call_quota_exceeded`, or `leaven.concurrency_limit` respectively.

For long-running calls (`agent.run`, `human.review`), the engine MUST enforce the cap mid-call if cost crosses it, terminating the call with `status: "cancelled"` and a final receipt that records the spend up to cancellation.

The lock spec §4.5 of the design notes records an open question on token-level aggregate USD caps. When that lands, this profile MUST honor `max_total_usd_micro` at the token scope alongside per-grant caps. Engines SHOULD implement aggregate metering even before the schema field exists, by summing per-grant cumulative spend.

### 11.3 Replay

Under `EvalMode.replay { receipts }` (plan.v1's mode discriminant), the engine MUST serve a receipt-matched call from the recorded value and MUST NOT dispatch to any provider. The match is by `request_hash` per `OperationReceiptV1` (kind `call`). A mismatch MUST be a `leaven.replay_mismatch` error; the worker MUST NOT silently fall through to live dispatch.

Replay MUST set `_meta.leaven_cache: "replayed"`, `_meta.leaven_receipt` to the original receipt reference, and MUST preserve the original redactions. Cost on a replayed receipt MUST be zero (the money was spent at original dispatch and is recorded there; double-charging is forbidden per lock spec §16).

---

## 12. Error model

The engine MUST map every tool-call error onto one of the following JSON-RPC error codes. These codes are normatively closed for v1; new codes require a profile version bump.

| Code (string)                            | Numeric range | When                                                                                          |
| ---------------------------------------- | ------------- | --------------------------------------------------------------------------------------------- |
| `leaven.unauthorized`                    | -32000        | The session is not authenticated (no ACP `authenticate` completed, or token revoked).         |
| `leaven.capability_missing`              | -32001        | The token has no grant for this tool's action.                                                |
| `leaven.capability_constraint_violation` | -32002        | The grant exists but the call arguments are outside its constraints.                          |
| `leaven.input_class_forbidden`           | -32003        | A declared input class is in the grant's `forbidden_input_classes`.                           |
| `leaven.input_class_not_allowed`         | -32004        | A declared input class is not in the grant's `allowed_input_classes`.                         |
| `leaven.input_classes_inconsistent`      | -32005        | `_meta.leaven_input_classes` and the IR-side `input_classes` disagree.                        |
| `leaven.budget_exceeded`                 | -32010        | Cumulative or projected USD spend would exceed the grant cap.                                 |
| `leaven.call_quota_exceeded`             | -32011        | `limits.max_calls` would be exceeded.                                                         |
| `leaven.concurrency_limit`               | -32012        | `limits.max_concurrent` would be exceeded.                                                    |
| `leaven.timeout`                         | -32020        | The call exceeded `timeout_s` or `limits.timeout_s`.                                          |
| `leaven.cache_required_miss`             | -32030        | `cache_policy: require_cached` and there is no cache entry.                                   |
| `leaven.replay_mismatch`                 | -32031        | Plan mode is `replay` but no receipt matches the call's request hash.                         |
| `leaven.async_pending`                   | -32040        | Human review (or other async) is pending; worker MUST poll via watch.                         |
| `leaven.provider_policy_denied`          | -32050        | The provider or engine policy refused the call (e.g., safety filter, model-pool restriction). |
| `leaven.provider_error`                  | -32051        | The provider returned an error that does not map to a Leaven-specific code.                   |
| `leaven.workspace_invalid`               | -32060        | The supplied `WorkspaceRef` is not a Leaven-issued handle or has been released.               |
| `leaven.runtime_unavailable`             | -32061        | The requested agent runtime is offline or not in the grant's `runtimes`.                      |
| `leaven.invalid_argument`                | -32602        | JSON-RPC -32602; the arguments fail `inputSchema` or IR-shape validation.                     |
| `leaven.internal_error`                  | -32603        | JSON-RPC -32603; engine-side fault.                                                           |

The engine MUST include `data._meta.leaven_redaction` (a `Redaction` object with a typed reason) on every denial code so audit trails capture *why* the call was refused without leaking secrets. The engine MUST also map the code into `PlanErrorV1.code` (per `leaven.plan_result.v1.schema.json` line 518; the lock spec design notes flag this as a closed enum the lock will pin) so a failed tool call surfaces consistently whether the worker sees the MCP error directly or fetches the receipted error through the graph.

Tool-execution errors that are not call-refusals (e.g., the LM provider returned content that failed `output.json_schema` validation, the sandbox exited non-zero, the agent session timed out from its own runtime) SHOULD surface as `tools/call` results with `isError: true` and a structured `content` payload describing the failure, plus the usual `_meta.leaven_receipt` so the worker can still record the (now failed) effect. This matches MCP `tools.mdx` lines 453-471: protocol errors for "the call cannot be made", execution errors for "the call ran and failed".

---

## 13. Provider policy integration

The engine's `lm.complete` and `agent.run` dispatch MAY apply provider-specific policy effects before, during, and after provider interaction:

- **Pre-dispatch safety filters**: the engine MAY refuse a call entirely (`leaven.provider_policy_denied`), e.g., when the message content trips a configured filter. The refusal MUST carry a `Redaction` with reason from the closed `Redaction.reason` enum (`secret | count_policy | path_denied | ...` per `common.schema.json`).
- **Output redaction**: the engine MAY scrub provider output (e.g., remove provider-internal moderation rationales, strip identifying metadata). Redactions appear in `_meta.leaven_redactions`; the redacted-out content is never observable to the worker by any path through this profile.
- **Model-pool restriction**: the grant's `LmCompleteGrant.constraints.models` and `model_roles` are enforced strictly. A worker that names a model outside the grant gets `leaven.capability_constraint_violation` before any provider sees the request.
- **Raw prompt/completion logging**: per `LmCompleteGrant.constraints.raw_prompt_logging` and `raw_completion_logging` (`forbidden | redacted | operator_only | full`), the engine MUST gate what survives in the receipt's recorded payload. Workers MUST NOT receive raw payloads when the policy is `forbidden` or `redacted`; the result fields `message` and `parsed` carry only what the policy allows.

Workers MUST treat provider policy as opaque and final. The engine is the policy boundary; there is no MCP-level escape hatch.

---

## 14. The fallback architecture in detail

This section is normative for engines implementing Section 4.2.

### 14.1 Process model

- Per ACP session, the engine MUST spawn exactly one MCP shim process (`leaven-mcp-server` or equivalent) OR run an in-engine MCP server on a per-session Unix socket. In either case, the per-session isolation invariant holds: no two ACP sessions share a single MCP endpoint, and no worker can address an MCP endpoint belonging to a different session.
- The MCP endpoint MUST be reachable to the worker via exactly one of:
  - A second stdio pair handed to the worker process at spawn (file descriptors inherited from the ACP-driving process or pipes whose paths are passed in env).
  - A Unix domain socket at a path under a 0700-mode per-session runtime directory; the socket itself MUST be mode 0600.

### 14.2 Token binding

- The engine MUST register the resolved capability grants with the MCP endpoint before the worker connects. Registration MUST include the bound principal (subject), all relevant grants, and the session identifier.
- The MCP endpoint MUST refuse any connection that does not present the agreed session identifier on first message. The session identifier is not a credential; it is a routing key. Trust derives from the engine's out-of-band registration.
- The engine MUST invalidate the registration when the ACP session ends, when the capability token expires, or on explicit revocation. After invalidation, the MCP endpoint MUST refuse all further `tools/call` requests with `leaven.unauthorized`.

### 14.3 Lifecycle

- The MCP shim MUST be running and ready before `session/new` returns to the worker. The engine MUST verify the shim has loaded the grant document before signaling readiness.
- The MCP shim MUST shut down when the ACP session closes. The engine MUST send SIGTERM, wait a bounded interval (SHOULD be 10 seconds), then send SIGKILL if the shim has not exited. The engine MUST log the shutdown as a `Redaction` reason `lifecycle_terminated` if any in-flight call was cancelled by the shutdown.
- If the MCP shim crashes mid-session, the engine MUST mark the ACP session as failed and propagate the failure to the worker via ACP. The engine MUST NOT silently restart the MCP shim and resume; in-flight effect receipts may be partially recorded and a clean session boundary is required.

### 14.4 Engine-internal channel

The engine-internal channel between the engine core and the MCP shim is out of scope for this profile (it may be gRPC, in-process Rust, an internal Unix socket, etc.). Implementations MUST ensure it is not exposed to the worker or to any non-engine principal. Grant document leakage through this channel is forbidden.

### 14.5 Realization-choice surfacing

The engine MUST surface the transport realization choice to the worker via the ACP session establishment metadata (e.g., as a field on the `session/new` response under `_meta.leaven_mcp_realization`). Permitted values:

- `acp` for Section 4.1.
- `stdio` for Section 4.2 with stdio transport.
- `socket` for Section 4.2 with Unix socket transport.

Workers MUST use this signal to connect on the correct channel and MUST NOT probe for endpoints.

---

## 15. What this profile does NOT cover

- **Engine-to-engine MCP federation.** Multiple Leaven engines sharing tool surfaces is out of scope. A Leaven engine is a single trust domain; cross-engine work uses the public IR over the same ACP profile, not nested MCP.
- **MCP resources and prompts.** The engine MUST NOT expose MCP resources, prompts, or completions surfaces at v1. Resources are not the right shape for graph reads (the ACP extension methods are typed and capability-aware in ways the resource model is not). Prompts are out of scope for an effect-callback channel.
- **Worker-to-worker MCP.** Workers do not call each other through Leaven's MCP surface. Composition between workers happens through the IR and the graph, not over MCP.
- **In-LM tool calling.** Tool calls *issued by the LM* during an `lm.complete` call (the `tools` array under `sampling/createMessage`) are not wired through this profile at v1. See Section 8.
- **Streaming LM completions.** The `lm.complete` tool returns one result. Streaming-style chunked completion is deferred to v1.1 and will use the watch protocol rather than MCP notifications.
- **Multimodal LM content.** `LmMessageV1` is text-only at v1 (`design_notes §4.10`); this profile inherits the limitation and does not add multimodal content types over MCP.
- **MCP-over-ACP v2 stability work.** This profile does not block on upstream RFD stabilization; Section 4.2 is a complete realization.

---

## 16. Compatibility and versioning

The profile inherits the lock spec's versioning discipline (§17):

- Pinned to MCP `2025-11-25` and ACP v1 (with optional v2 MCP-over-ACP for Section 4.1).
- The four tool names, their capability-action mapping, their `inputSchema`/`outputSchema` discriminants, and the `_meta` envelope keys are stable for v1.
- New tools MAY be added in v1.x only when the underlying lock spec adds them (e.g., if the lock spec adds a `code.format` or `web.search` Call kind that warrants engine-managed dispatch). Adding a tool MUST be an additive change with a documented capability action.
- Removing or renaming a tool requires a new major profile version.
- New error codes MAY be added in v1.x but MUST NOT shadow existing codes' semantics.
- New `_meta` keys MUST use the `leaven_` prefix. Worker SDKs MUST preserve unknown `leaven_` keys when forwarding (e.g., when an intermediary relays tool results into a higher-level observer).

The engine MUST advertise its profile version under `_meta.leaven_mcp_profile_version: "1"` on `tools/list`. Worker SDKs MUST refuse to connect when the value is not `"1"` and MUST NOT default-trust higher major versions until they have explicit compatibility logic.

The profile tracks MCP spec evolution: when MCP `2025-11-25` is superseded upstream, this profile MAY adopt a successor revision in a v1.x update *iff* the upgrade is wire-compatible on the four tool surfaces. Otherwise the upgrade is a major version bump.

---

## 17. Risks and watchitems

17.1 **MCP-over-ACP v2-unstable upstream.** The RFD and v2 module are explicitly marked UNSTABLE. Section 4.2 fallback exists for exactly this reason. Watch for incompatible RFD revisions; do not let the unstable realization become the only realization until upstream stability is declared.

17.2 **MCP version drift.** A profile pinned to `2025-11-25` will eventually face upstream MCP changes. The risk is biggest at the `tools/call` envelope shape and the `structuredContent` mechanism; both are central to this profile. The compatibility note in §16 must be enforced in the engine's CI against current MCP schema.

17.3 **Sampling-shape divergence from `LmCompleteCallV1`.** Because we do not use `sampling/createMessage` for LM dispatch (Section 8), we are not bound to its evolution. But adapters that bridge external MCP servers using `sampling/createMessage` into Leaven `lm.complete` will accumulate drift if the two shapes diverge meaningfully. The lock spec's open question §4.10 (multimodal, in-LM tool calling, streaming) interacts here: when we adopt multimodal or in-LM tool calling in `lm.complete`, we MUST align with MCP's `sampling/createMessage` shape where the cognate exists, even though we do not use that method directly.

17.4 **Dual-channel-spawn complexity (Section 4.2).** The fallback requires the engine to manage a second process or a second socket per worker session, plus out-of-band token registration with that endpoint. Misconfiguration could expose an MCP endpoint to processes outside the worker's trust boundary. Engines MUST enforce strict per-session isolation, mode-0600 socket permissions, env-var-only credential routing, and aggressive teardown on session end.

17.5 **Capability-token-to-MCP-server-trust handoff correctness.** The fallback's out-of-band grant registration is a security-critical interface. A bug here (e.g., serving a request before the registration completes, or holding stale grants after token revocation) breaks the capability model. Engines MUST have automated tests covering: pre-registration call refusal, post-revocation call refusal, mid-session capability narrowing (attenuation), and TOCTOU between registration update and concurrent in-flight call.

17.6 **Per-tool replay-class accuracy.** The `_meta.leaven_replay_class` MUST be honest. An engine that records `fully_managed` for a call that actually leaked into an untracked external effect (e.g., a sandbox that escaped network isolation) violates a core lock-spec invariant. Replay-class assertion belongs in the engine's effect-receipt-generation path, not in the worker SDK.

17.7 **Long-running call cancellation.** `agent.run` and `human.review` can run for hours. The cancellation pathway from ACP `session/cancel` into mid-flight MCP `tools/call` execution must be robust. The engine MUST ensure that ACP cancellation propagates to the MCP shim (in the Section 4.2 realization) and to the in-flight provider client without leaving zombie processes or stuck reservations.

17.8 **`structuredContent` rendering churn.** MCP's `structuredContent` vs unstructured `content` story is evolving in the spec. The profile pins both: `structuredContent` is the source of truth; `content[0]` carries the same value serialized for backwards compatibility. If MCP makes `structuredContent` mandatory and `content` optional (or vice versa), the profile may need an additive v1.x clarification.

17.9 **`_meta` propagation discipline.** ACP, MCP, and Leaven all use `_meta` for extensibility. Workers and engines MUST avoid namespace collision: Leaven keys MUST start with `leaven_`; ACP-defined keys belong to the ACP profile; MCP-defined keys (annotations on content, etc.) belong to MCP. An engine that emits a non-prefixed Leaven-meaningful key in `_meta` is non-conformant.

---

## 18. Open questions

These are gaps deliberately surfaced rather than papered over. Each one has a working interim choice; the production answer is pending lock-spec or RFD evolution.

18.1 **`human.review` value kind in `plan_result.v1`.** The lock spec's `leaven.plan_result.v1.schema.json` currently models human review under the generic write-receipt shape with `kind: "human_review"` is not yet a first-class value. Section 7.4 prescribes the intended shape; the schema needs the field. Until it lands, engines MUST return a value with `kind: "human_review"` and the fields documented in 7.4, and SHOULD treat the receipt as the canonical record.

18.2 **`PlanErrorV1.code` enum closure.** Section 12 pins a closed error-code enum on the MCP wire. The lock spec's `PlanErrorV1.code` is currently `string` (design notes issue 59). When the lock spec closes it, this profile's table must align exactly. Engines SHOULD pre-emptively limit their code emission to the table here.

18.3 **In-LM tool calling.** MCP's `sampling/createMessage` carries a `tools` array enabling LM-driven tool calling. Section 8 defers this to v1.1. Open question: do we surface in-LM tool calling as an extended `lm.complete` argument, or as a separate `lm.tool_chat` tool, or by aligning `lm.complete` with the full `sampling/createMessage` shape under an additive `_meta.leaven_supports_in_lm_tools: true`? The current call: defer; revisit once `LmCompleteCallV1` answers design notes §4.10.

18.4 **Streaming completions.** Section 15 defers streaming to the watch protocol. The watch protocol is itself under-specified (design notes §4.9). Until both are decided, workers requiring token-by-token streams MUST instead accept a single `lm.complete` result. If a near-term streaming need emerges, MCP `notifications/message` log notifications on the connection MAY carry partial completions as additive telemetry, with the final canonical value still in the `tools/call` result.

18.5 **Aggregate USD budgeting across grants.** Lock spec design notes §4.5. Section 11.2 SHOULD-implements aggregate metering, but the schema field is missing. The profile honors per-grant caps; aggregate behavior is implementation-defined until the schema closes the question.

18.6 **MCP-over-ACP RFD stability.** Section 4.1 depends on upstream stabilization. If upstream changes incompatibly (e.g., renames `mcp/connect`, restructures `mcp/message`, changes `connectionId` semantics), engines using 4.1 MUST downgrade to 4.2 until the profile is updated. Section 17.1 names the risk; this is the operational decision.

18.7 **Provider-side tool use vs Leaven-managed sandbox.** When an `lm.complete` call uses an `output` of `kind: "files"` or similar that implies the LM produced workspace-affecting output, the boundary between "the LM proposed an edit" and "the engine should also run a sandbox check" is policy. v1 keeps the boundary at the worker: the worker decides whether to run a sandbox check after receiving the LM completion. If the lock spec eventually adds an `output: workspace_diff` contract on `lm.complete`, this profile must extend `_meta` to surface workspace materialization receipts from the completion.

18.8 **`provider_hints` semantics.** `LmCompleteCallV1.provider_hints` is currently open (design notes issue 21). Until pinned, this profile treats it as an opaque per-provider key-value bag; the engine MUST NOT make security decisions based on `provider_hints` content alone.

---

## 19. Implementation checklist (engine side)

This is a non-normative summary. Items map to the lock spec's §18 implementation checklist; this profile contributes the MCP-side tasks.

- Implement the tool registry exposing exactly `lm.complete`, `agent.run`, `sandbox.exec`, `human.review` with the schemas in Section 7.
- Wire `tools/call` requests through the same authorization kernel used by the ACP extension methods (lock spec §6.6).
- Validate `_meta.leaven_input_classes` against the grant before any dispatch.
- Implement both Section 4 realizations OR Section 4.2 alone; surface the realization choice via ACP session metadata.
- Implement the Section 12 error-code table; verify each code path emits a typed `Redaction`.
- Implement cache, budget, and replay per Section 11; verify replay-by-receipt does not dispatch to providers.
- Provide per-tool integration tests covering: authorized success, capability-missing denial, constraint-violation denial, input-class denial, budget-cap mid-call cancellation, replay match, replay mismatch, cache hit/miss/required-miss/bypass, async pending (human review).
- Provide a worker-side conformance check that an engine's `tools/list` matches this profile exactly.

---

## 20. Compliance statement

An engine is conformant with this profile when:

1. It implements at least the Section 4.2 fallback realization with the credential and lifecycle invariants of Section 14.
2. It exposes exactly the four tools in Section 7 with the documented input/output schemas, capability actions, and `_meta` envelope.
3. It enforces capability-token authorization per Section 5 on every `tools/call`.
4. It returns errors only from the Section 12 closed set, with `Redaction` reasons.
5. It honors cache, budget, and replay semantics per Section 11.
6. It does not expose forbidden tools (Section 7.5) over MCP.
7. It advertises `_meta.leaven_mcp_profile_version: "1"` on `tools/list`.

A worker SDK is conformant when:

1. It connects on the realization signaled by the engine via ACP session metadata.
2. It calls only the four documented tools.
3. It declares `_meta.leaven_input_classes` on every call and keeps it equal to the IR-side `input_classes` field when both are present.
4. It treats Section 12 error codes as terminal for the call and does not silently fall through.
5. It preserves unknown `leaven_*` `_meta` keys when forwarding tool results into observability or higher-level orchestration.

This profile is forever once locked. Implementations build against it directly; integrations bridge in and out of it through this surface.
