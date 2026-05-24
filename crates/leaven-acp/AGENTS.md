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
  writes/reads, live progress-update handling, cancellation notification, and
  subprocess cleanup.
- `lib.rs` is a map only.

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
  cancellation delivery to a live worker.

## Verification

- Run `cargo test -p leaven-acp` after changing transport/session behavior.
- Run `cargo test -p leaven --test topology_contract` after changing this
  crate's dependencies, facade routing, or workspace topology.
