# Public Seam V1 ACP Transport Route

Status: planning note, not implemented proof
Created: 2026-05-24T18:46:40Z

## Purpose

This note records the implementation route for the blocked public-seam V1 ACP
rows without changing the locked spec semantics in
`docs/specs/public-seam-v1`.

The immediate risk is a false positive: using Rust in-process calls or a
provider-specific adapter, calling it ACP, and then marking ACP rows proven.
That is not acceptable. The conformance matrix already names this fake pass for
`ps1.acp.transport_profile`.

## Boundary Decision

`crates/leaven-public-seam` remains the locked V1 public seam wire-contract
owner. It owns active package loading, schema/profile validation, Leaven
`leaven/*` method validation, capability documents, grant checks, Plan IR
params, Plan Result and ACP extension-result envelopes, redactions, receipts,
data-class projection checks, and matrix harness data.

`crates/leaven-public-seam` must not become a process runtime, ACP I/O loop,
worker supervisor, provider adapter, graph mutation owner, or schema-codegen
substitute.

The missing ACP rows need a hot agent/worker transport owner. If implemented as
a new crate, the expected shape is a transport adapter such as
`leaven-public-seam-acp` or `leaven-agent-acp`. The final name should be chosen
when the crate is added, but the ownership should stay narrow:

- depend on `agent-client-protocol` for ACP stdio JSON-RPC, process/session,
  request/notification, and cancellation/update substrate;
- depend on `leaven-public-seam` for the locked Leaven method/profile/result
  contract;
- call into engine/run/agent/workspace/LM owners through their public authority
  paths;
- never mutate `RunGraph` except through `RunContext`-owned operations;
- never enable or emulate MCP-over-ACP for V1.

## ACP Versus Codex App-Server

Codex app-server is a concrete provider runtime path for Leaven agent stages.
It is owned by `leaven-agent-codex-app-server`, behind explicit features, and
maps Codex app-server protocol events into provider-neutral `leaven-agent`
session facts.

ACP is different. It is the locked external-language worker public seam for V1.
It proves that a worker process outside the Rust process can participate in a
Leaven run over the public protocol boundary.

Therefore:

- Leaven does not need ACP in order to use Codex app-server as its primary
  Codex provider runtime.
- Codex app-server success does not prove the ACP public seam.
- ACP success does not replace Codex app-server provider tests.
- A Codex-backed worker can count as ACP evidence only if it runs through the
  ACP transport adapter, crosses the JSON-RPC process boundary, authenticates
  through the Leaven capability route, and returns locked Leaven extension
  results through `leaven-public-seam` validation.

## SDK Decision

Use the official `agentclientprotocol/rust-sdk` checkout as the ACP substrate
for the hot transport owner. The local reference checkout inspected for this
decision is:

`/Users/darin/vendor/github.com/agentclientprotocol/rust-sdk`

The relevant SDK facts are:

- `agent-client-protocol` provides roles, builders, handlers, protocol schema
  types, and JSON-RPC infrastructure.
- `Stdio` provides ACP stdio transport over stdin/stdout.
- `AcpAgent` can spawn an external ACP process and wire stdio.
- The SDK exposes request/notification handler registration for typed and
  untyped dispatch.
- MCP-over-ACP support is behind unstable feature gates and is not part of
  Leaven V1.

The adapter must treat the SDK as transport plumbing, not as the Leaven domain
contract. Leaven still owns method set, capability mapping, result envelopes,
receipts, redactions, data-class checks, and row evidence.

## First Implementation Tranche

The smallest honest tranche is a black-box subprocess harness that proves one
real ACP session over stdio JSON-RPC.

Suggested structure:

- Add a hot ACP transport crate or module with a local `AGENTS.md` if it is a
  new crate.
- Add a tiny test worker binary or fixture that speaks ACP over stdio as a
  separate child process.
- The engine/client side launches the child with
  `LEAVEN_CAPABILITY_TOKEN`, `LEAVEN_ENDPOINT`, and
  `LEAVEN_CAPABILITY_FINGERPRINT`.
- The worker authenticates through ACP and receives a Leaven capability-bound
  session.
- A locked `leaven/*` extension method crosses the JSON-RPC boundary with Plan
  IR params validated by `leaven-public-seam`.
- The child returns an ACP extension-result envelope validated by
  `leaven-public-seam`.
- The test observes that the result was carried over the ACP process/session
  route, not produced by an in-process shortcut.

This first tranche should not claim all ACP rows proven unless it covers every
method family and lifecycle fact named by the row. It can still be valuable as
partial evidence.

## Required Negative Tests

The transport tranche must include negatives that kill realistic false
implementations:

- Rust in-process trait calls labeled as ACP.
- Bare method-specific payload returned over ACP without the result envelope.
- Cross-method or wrong-kind response accepted for a request.
- Missing, expired, revoked, or fingerprint-mismatched authenticate path.
- Permission bypass that uses a raw `CapabilityDocument` without authenticated
  ACP session state.
- MCP-over-ACP method, negotiation, bridge, or unstable SDK feature enabled in
  V1.
- Worker progress updates that grow without the bounded queue policy.
- Cancellation accepted as a marker but not bound to live worker activity.
- Extension result that passes JSON Schema but fails Leaven semantic validation
  for receipts, redactions, capability fingerprint, data classes, or result
  hash.

## Row Policy

Do not change any matrix row from `blocked` or `pending` to `proven` from this
planning note.

Rows that can receive partial evidence only after implementation:

- `ps1.acp.transport_profile`
- `ps1.acp.extension_results`
- `ps1.acp.lifecycle_backpressure`

Before any of those rows is promoted, require:

- executable positive and negative evidence matching the row proof fields;
- black-box subprocess proof for any claim about ACP process/session behavior;
- no MCP-over-ACP route in V1;
- adversarial review focused on spec drift, fake passes, missing negatives,
  topology leaks, and public-maturity overclaiming;
- matrix evidence updated only after reviewer findings are resolved or recorded
  as non-blocking.

## Verification Gates

During implementation:

- Run the new ACP transport crate's focused tests.
- Run `cargo test -p leaven-public-seam --test acp_profile` when touching the
  Leaven ACP profile or extension-result validation.
- Run `cargo test -p leaven --test topology_contract` when adding the crate,
  dependencies, facades, features, or public routes.
- Run `just check` before claiming completion of implemented behavior.

## Open Questions

- Final crate name: `leaven-public-seam-acp` versus `leaven-agent-acp`.
- Whether the official SDK's typed extension hooks can represent Leaven's
  exact `leaven/*` method names directly, or whether the adapter should use
  lower-level untyped dispatch.
- The smallest method family that should be used for first black-box proof.
- Whether the first subprocess fixture should be a Rust test worker or an
  external-language worker to better match the public-seam promise.
