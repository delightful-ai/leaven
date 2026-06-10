# Leaven Worker Profile v1 — v0.3

This file has a legacy path/name. The current V1 contract is the Leaven worker
profile for the public seam: a Leaven-owned stdio JSON-RPC protocol for
external-language workers. It is not upstream Agent Client Protocol
conformance, and V1 does not depend on an upstream ACP SDK.

Leaven is the domain protocol and the public API seam.

The worker profile is the process/session transport binding for that seam.

The engine side owns run state and dispatches stages/effects across the seam.

The worker side runs external-language stage code and calls back to engine-owned
effects through `leaven/*` methods.

The inversion from ordinary parent/child process intuition must be documented in
SDKs: the process that was spawned may still initiate `leaven/stage.run`, and
the parent may still initiate `leaven/lm.complete` callbacks, depending on the
spawn direction.

The profile pins a Leaven worker protocol version at implementation release time.

The profile supports stdio JSON-RPC first.

Unix socket, HTTP JSON-RPC, and WebSocket JSON-RPC may be added by transport binding.

Authentication maps to `leaven.capability.v1`.

For stdio, the engine spawns workers with `LEAVEN_CAPABILITY_TOKEN`, `LEAVEN_ENDPOINT`, and `LEAVEN_CAPABILITY_FINGERPRINT`.

For HTTP/WebSocket, the bearer token rides `Authorization: Bearer`.

Worker-profile authentication resolves the opaque token to a capability document.

The bearer secret is never persisted in run artifacts.

Permission requests are answered programmatically by the capability grant.

Human approval is an operator policy, not the default authorization engine.

Denials return `PlanError` and `Redaction` information.

All worker lifecycle cancellation uses Leaven worker-session cancellation.

All progress uses Leaven worker-session updates.

Leaven implementations must use bounded update queues.

Unbounded update queues are forbidden in production Leaven workers.

Leaven extension methods cover the full worker callback surface.

The locked profile defines the method denominator. Current executable service
availability for `leaven seam serve --stdio` is recorded in
`../executable-method-status.md`; do not infer service readiness only from a
method being present in this profile.

Stage dispatch (engine to worker): `leaven/stage.run`. The engine dispatches one stage to the worker as a single generic method carrying a stage kind plus a role-scoped stage payload, and the worker returns that stage's typed output. V1 dispatches the target-free runner stage, the scorer stage (which reads the scored case through capability-gated case access and returns a typed reward-vector score alongside its text output), and the proposer stage. This is the inverse direction from the callbacks below: here the engine asks the worker to run a stage rather than the worker asking the engine to perform an effect. `leaven/stage.run` binds the dedicated `leaven.stage_run.v1` request and result schemas, not the Plan IR effect schemas the callbacks use.

Client dispatch (client to host): `leaven/optimize.run`. This is the one method an SDK client uses to request a full optimization run from the host; the host owns the optimizer loop and the durable run state. It is a third method direction, distinct from the host->worker stage dispatch above and the worker->host callbacks below: the client asks the host to run an optimization, not the host asking a worker to run a stage and not a worker asking the host to perform an effect. Because it is not part of the worker callback surface, the host does not advertise `leaven/optimize.run` as one of the worker `extension_methods` and a worker process never serves it. `leaven/optimize.run` binds the dedicated `leaven.optimize_run.v1` request and result schemas: the request carries a seed prompt artifact record, a target-bearing case manifest (the host owns target custody), optimizer config, and reflection config; the result returns the optimized projection (best candidate, frontier with lineage and scores, iteration and metric-call counts, cost totals, the durable run/revision reference, and applied proposal-batch receipts). Current configured service execution status is recorded in `../executable-method-status.md`.

Graph operations: `leaven/graph.query`, `leaven/case.load`, `leaven/case.input`, `leaven/case.target`, `leaven/case.metadata`.

Workspace operations: `leaven/workspace.materialize`, `leaven/workspace.snapshot`, `leaven/workspace.list`, `leaven/workspace.read_file`, `leaven/workspace.stat`, `leaven/workspace.digest`, `leaven/workspace.git_log`, `leaven/workspace.git_diff`, `leaven/workspace.git_status`, `leaven/workspace.capture_artifacts`, `leaven/workspace.release`.

Costful effects: `leaven/lm.complete`, `leaven/agent.run`, `leaven/sandbox.exec`.

Graph mutations: `leaven/proposal.submit_batch`, `leaven/proposal.apply`, `leaven/assessment.submit`, `leaven/evaluation.request`, `leaven/event.emit`.

Watch placeholders are present but deferred to v1.1.

Each extension method declares required capability action.

Each extension result returns primary value plus receipts, redactions, capability fingerprint, and data classes.

There is no upstream ACP compatibility layer and no MCP bridge layer in v1.

All worker callbacks ride Leaven `leaven/*` seam methods uniformly.

A future v1.x may add explicit interoperability adapters for upstream ACP or MCP
clients, but neither is required to use the Leaven public seam.
