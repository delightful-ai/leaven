# Changelog v1 (from v0.3 draft)

Reorganized the bundle by abstraction plane.

Replaced `worker_protocol` with the Leaven ACP profile.

Dropped MCP-over-ACP from v1. All worker callbacks (LM dispatch, agent runs, sandbox exec, graph queries, case reads, workspace operations, proposal/assessment writes, evaluation requests, and event emission) ride ACP extension methods uniformly. A future v1.x may add an MCP bridge for interoperability with non-Leaven MCP clients, but it is not required to use the seam.

Deferred `watch.v1` to v1.1.

Added `ReflectionResultV1`.

Added `ProposeRequestV1`.

Made `Score.output` required in common score shape.

Unified evidence visibility enum.

Pinned schema fingerprinting to RFC 8785 JCS + SHA-256.

Pinned field paths to RFC 6901 JSON Pointer.

Pinned extraction to RFC 9535 JSONPath Leaven subset.

Pinned templates to `leaven.mustache.strict.v1`.

Added aggregate capability budgets.

Added token `jti`, subject fingerprint, token binding, revocation, renewal, and expiry behavior.

Added data-class propagation rules to common spec and prose.

Added typed graph rows and typed plan-result values.

Added first-class workspace handles.

Added call result hashes.

Added timestamps to receipts.

Closed plan error codes.

Typed write receipt IDs by write kind.

Added per-assessment replayability.

Added `ChangeFromAgentSession`.

Added workspace release.

Added `git_status`.

Added typed LM tools and tool-call IDs.

Added sandbox streaming policy.

Added one generic host-to-worker stage-dispatch method `leaven/stage.run` bound to `leaven.stage_run.v1` (request: stage kind plus a role-scoped stage payload; result: the stage output), separate from the Plan IR effect callbacks. V1 dispatches the target-free runner stage and returns a text `OutputRecord`.
