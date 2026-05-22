# Leaven ACP Profile v1 — v0.3

ACP is the worker lifecycle protocol.

Leaven is the domain protocol.

The engine is the ACP client.

The worker is the ACP agent.

The inversion from IDE-client/coding-agent to engine-client/stage-worker-agent must be documented in SDKs.

The profile pins an ACP version at implementation release time.

The profile supports stdio JSON-RPC first.

Unix socket, HTTP JSON-RPC, and WebSocket JSON-RPC may be added by transport binding.

Authentication maps to `leaven.capability.v1`.

For stdio, the engine spawns workers with `LEAVEN_CAPABILITY_TOKEN`, `LEAVEN_ENDPOINT`, and `LEAVEN_CAPABILITY_FINGERPRINT`.

For HTTP/WebSocket, the bearer token rides `Authorization: Bearer`.

ACP `authenticate` resolves the opaque token to a capability document.

The bearer secret is never persisted in run artifacts.

ACP permission requests are answered programmatically by the capability grant.

Human approval is an operator policy, not the default authorization engine.

Denials return `PlanError` and `Redaction` information.

All worker lifecycle cancellation uses ACP session cancellation.

All progress uses ACP session updates.

Leaven implementations must use bounded update queues.

Unbounded update queues are forbidden in production Leaven workers.

Leaven extension methods cover the full worker callback surface.

Graph operations: `leaven/graph.query`, `leaven/case.load`, `leaven/case.input`, `leaven/case.target`, `leaven/case.metadata`.

Workspace operations: `leaven/workspace.materialize`, `leaven/workspace.snapshot`, `leaven/workspace.list`, `leaven/workspace.read_file`, `leaven/workspace.stat`, `leaven/workspace.digest`, `leaven/workspace.git_log`, `leaven/workspace.git_diff`, `leaven/workspace.git_status`, `leaven/workspace.capture_artifacts`, `leaven/workspace.release`.

Costful effects: `leaven/lm.complete`, `leaven/agent.run`, `leaven/sandbox.exec`, `leaven/human.review`.

Graph mutations: `leaven/proposal.submit_batch`, `leaven/proposal.apply`, `leaven/assessment.submit`, `leaven/evaluation.request`, `leaven/event.emit`.

Watch placeholders are present but deferred to v1.1.

Each extension method declares required capability action.

Each extension result returns primary value plus receipts, redactions, capability fingerprint, and data classes.

There is no MCP-over-ACP layer in v1.

All worker callbacks ride ACP extension methods uniformly.

A future v1.x may add an MCP bridge for interoperability with non-Leaven MCP clients, but it is not required to use the seam.
