# 01 — Plan IR Spec v0.3

`leaven.plan.v1` has three node classes: `Let`, `Call`, `Write`.

`Let` is pure.

`Call` is effectful.

`Write` is staged graph mutation intent.

`Let` nodes can query graph, cases, and workspace snapshots.

`Call` nodes can invoke LM, agent, sandbox, human review, workspace materialization, and extensions.

`Write` nodes can submit proposals, submit assessments, request evaluations, apply proposals, emit events, and invoke extension writes.

The query expression is flattened.

There is no `expr.graph_query.query.source` nesting.

Graph sources use semantic names such as `by_candidate` to avoid discriminator collision.

Field paths and redaction paths use RFC 6901 JSON Pointer.

Extraction uses RFC 9535 JSONPath, Leaven subset.

Templates use `leaven.mustache.strict.v1`.

Every executable mini-language is pinned.

Unpinned syntax is a replay hazard.

Case set expressions include union, intersect, difference, sample, and stratified.

Explicit case IDs require partition resolution.

Tags require partition resolution.

Recent sets require partition resolution.

Syntax is not authorization.

`lm_complete` supports text, tool messages, tool definitions, sampling, provider hints, final-message output, and JSON-schema output.

Multimodal and streaming LM output are extension/v1.1 concerns.

`agent_run` is separate from `lm_complete`.

`agent_run` returns session receipts, transcript refs, command refs, outputs, costs, and policy fingerprints.

`sandbox_exec` is v1.

`sandbox_exec` supports streaming policy and blob refs for stdout, stderr, and files.

Workspace materialization is a call.

Workspace reads are expressions.

Workspace release is a call.

Proposal batch semantics are explicit: `alternatives` or `sequence`.

`sequence` means proposal i+1 is intended against the candidate produced by proposal i.

`alternatives` means proposals share a causal base.

`ChangeFromAgentSession` is a first-class proposal effect.

Assessment writes require `Score.output`.

Assessment writes carry per-assessment replayability.
