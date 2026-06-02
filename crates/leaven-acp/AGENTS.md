## Boundary

This crate has a legacy name. In V1 it owns hot Leaven worker stdio
process/session transport behavior for the locked Leaven public seam. It does
not implement upstream Agent Client Protocol conformance and does not depend on
an upstream ACP SDK.

It starts external worker processes, carries line-framed JSON-RPC over
stdin/stdout, binds the profile-derived engine/worker lifecycle facts to that
live process, and delegates all Leaven method, Plan IR, Plan Result, receipt,
redaction, data-class, and capability-envelope validation to
`leaven-public-seam`.

It is not a provider runtime, graph mutation layer, engine `RunContext`, MCP
bridge, schema-codegen crate, LM client, concrete sandbox backend, or agent
provider adapter.

## Map

- `stdio` owns process spawning, launch environment projection, JSON-RPC line
  writes/reads, the demultiplexing read loop, live progress-update handling,
  worker-initiated effect-callback servicing, host->worker stage dispatch,
  cancellation notification, and subprocess cleanup.
- `AcpStdioSession<R, W>` is the legacy-named generic demultiplexing transport core over one
  line-framed reader/writer pair. It owns every shared transport leg
  (`call_extension`, `dispatch_stage_run`, `serve_next_inbound_request`,
  cancellation, session updates) so the same client loop runs unchanged over a
  spawned child or inherited process stdio. Specializations must reuse this core,
  not re-implement the demux.
  - `AcpStdioProcessSession` specializes it over a spawned child's piped
    stdin/stdout (the engine spawns the worker) and adds child cleanup/exit.
  - `AcpStdioInheritedSession` specializes it over the process's own inherited
    stdin/stdout for the inverse spawn direction: the parent spawned this process
    (for example `leaven serve --stdio`) and injected the locked capability env,
    so no child is launched. The engine side dispatches to the parent over
    inherited stdout and services the parent's callbacks from inherited stdin.
- `lib.rs` is a map only.

## Bidirectional Transport
The read loop is a demultiplexer: every inbound line classifies as a
`session/update` notification, a host→worker response by id, or a
worker-initiated Leaven request. Worker-initiated requests are validated as locked Plan IR
through `validate_acp_jsonrpc_request_document` (which gates the method through
the profile, rejecting private/MCP inbound exactly as the host→worker
direction), dispatched to the `AcpEffectHost` trait, validated as an extension
result through `validate_acp_extension_result_document`, stamped with the
launched session capability fingerprint, and written back under the worker's id.

- `AcpEffectHost` lowers a validated inbound request into a Leaven extension
  result. It owns no graph mutation, transport framing, or JSON-RPC ids. Only
  `lm_complete` is wired in V1; every other locked method rejects through the
  default `service` dispatch. Graph-write effects must route through
  `RunContext` finalizers in the engine, never here.
- Host ids (`leaven-acp-{n}`) and worker-originated ids never collide: the demux
  keys host→worker responses by the outstanding request id and answers
  worker-initiated requests under the worker's own id.
- The launched capability fingerprint is authoritative. The transport stamps it
  onto every inbound reply and refuses a host result that asserts a different
  fingerprint, so a host lowering can never answer on behalf of another session.

`dispatch_stage_run` is the host->worker stage-dispatch leg: it sends one
`leaven/stage.run` request validated through the stage-run JSON-RPC envelope
(not Plan IR), shares the same demultiplexing read loop so worker-initiated
`leaven/lm.complete` callbacks are serviced while the dispatch is in flight, and
validates the worker's reply as a locked stage-run result. The stage-run wire
truth (schema, profile binding, envelope validators) stays in
`leaven-public-seam`; this method only carries the validated envelope.

## Route Away

- Locked profile/schema/matrix truth stays in `leaven-public-seam`.
- Graph mutation stays in `leaven-engine` through `RunContext`.
- Provider execution stays in `leaven-lm*`, `leaven-agent*`, and workspace
  backends.
- Upstream ACP agent interop is not this crate's current V1 responsibility. If
  Leaven later uses upstream ACP to swap agent runtimes, that belongs behind an
  explicit agent-provider adapter slice, not by relabeling this worker seam.
- MCP bridge behavior is not V1 and must not appear in default/product paths here.

## Public Maturity

This crate is an advanced public seam transport contract. It proves the V1
external-worker process boundary over stdio JSON-RPC and black-box subprocess
tests. It is not re-exported by `leaven`, `leaven::prelude`, default features,
or product examples as ordinary app-facing API.

The locked V1 semantics are stdio JSON-RPC plus Leaven `leaven/*` envelopes.
Do not fetch upstream ACP crates, add an ACP SDK dependency, or change the
dependency choice without a fresh design slice that explains whether the goal is
agent-provider interop or worker-seam transport.

## Proof Anchors

- `crates/leaven-acp/tests/stdio_session_contract.rs` proves live subprocess
  transport, extension-result envelopes across the process boundary, private
  protocol rejection, bare payload rejection, bounded update queues, and
  cancellation delivery to a live worker. It also proves the inbound leg: a
  Python worker that *initiates* `leaven/lm.complete` and the host responds
  (`stdio_session_services_python_worker_initiated_lm_complete_request`), the
  inbound private/MCP rejection
  (`stdio_session_rejects_private_and_mcp_inbound_worker_requests`), and the
  foreign-fingerprint refusal
  (`stdio_session_rejects_inbound_host_result_with_foreign_capability_fingerprint`).

## Verification

- Run `cargo test -p leaven-acp` after changing transport/session behavior.
- Run `cargo test -p leaven --test topology_contract` after changing this
  crate's dependencies, facade routing, or workspace topology.
