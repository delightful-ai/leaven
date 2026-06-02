## Boundary

This crate owns hot ACP stdio process/session transport behavior for the
locked Leaven public seam. It starts external worker processes, carries
line-framed JSON-RPC over stdin/stdout, binds the profile-derived
engine-client/worker-agent lifecycle facts to that live process, and delegates
all Leaven method, Plan IR, Plan Result, receipt, redaction, data-class, and
capability-envelope validation to `leaven-public-seam`.

It is not a provider runtime, graph mutation layer, engine `RunContext`, MCP
bridge, schema-codegen crate, LM client, concrete sandbox backend, or agent
provider adapter.

## Map

- `stdio` owns process spawning, launch environment projection, JSON-RPC line
  writes/reads, the demultiplexing read loop, live progress-update handling,
  worker-initiated effect-callback servicing, host->worker stage dispatch,
  cancellation notification, and subprocess cleanup.
- `lib.rs` is a map only.

## Bidirectional Transport
The read loop is a demultiplexer: every inbound line classifies as a
`session/update` notification, a host→worker response by id, or a
worker-initiated extension request (the worker is the ACP agent; the engine is
the ACP client). Worker-initiated requests are validated as locked Plan IR
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
- MCP-over-ACP is not V1 and must not appear in default/product paths here.

## Public Maturity

This crate is an advanced public seam transport contract. It proves the V1
external-worker process boundary over stdio JSON-RPC and black-box subprocess
tests. It is not re-exported by `leaven`, `leaven::prelude`, default features,
or product examples as ordinary app-facing API.

The vendored `agentclientprotocol/rust-sdk` remains the preferred future ACP
substrate, but this crate currently avoids that dependency because the local
checkout requires uncached crates.io packages. The locked V1 semantics are
still stdio JSON-RPC plus Leaven `leaven/*` extension envelopes; do not fetch
external crates or change that dependency choice without the user's approval.

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
