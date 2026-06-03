## Boundary

This crate is the host-side bridge that composes the locked public seam into a
runnable prompt-optimization slice: it projects a runner rollout into a
`leaven/stage.run` dispatch over the `leaven-acp` transport, services the
worker's `leaven/lm.complete` callbacks against a deterministic host LM, parses
the worker's text stage output, and runs a tiny but real GEPA-shaped accept loop
that produces an `Optimized` `PromptArtifact`.

It composes three things it does not own:

- transport mechanics (`leaven-acp`) — process spawn, framing, the demultiplexing
  read loop, capability-fingerprint stamping;
- wire-contract truth (`leaven-public-seam`) — stage-run request/result schemas,
  the JSON-RPC stage-run envelope validators, the `lm_response` extension result;
- the prompt/LM/exact-match scenario it runs (example 03).

## Route Away

- GEPA search policy (parent Pareto frontier, component selection, reflective
  dataset construction, merge) belongs in `leaven-gepa`. The tiny accept loop
  here is a slice-3 product-proof, not a second optimizer home; do not grow it
  into GEPA's strategy state.
- Wire schemas, profile rows, and stage-run validation belong in
  `leaven-public-seam`. Do not re-encode the stage-run shapes here.
- Transport framing, ids, and the inbound demux belong in `leaven-acp`. This
  crate calls `dispatch_stage_run` / `call_extension`; it does not reach into the
  read loop.
- Concrete LM/agent/sandbox provider runtimes belong in `leaven-lm-*` /
  `leaven-agent-*`. The `HostLm` here is a deterministic mock; a live provider
  implements the same trait behind an explicit opt-in.
- Graph mutation belongs in `leaven-engine` through `RunContext`. The slice-3
  accept loop is an in-memory candidate selection over real rollout scores; it
  writes no `RunGraph`. When a later slice persists candidates, route the writes
  through `RunContext::propose` / `evaluate`, never raw contexts.

## Bidirectional Seam

This crate uses the legacy-named `leaven-acp` transport in its bridge-demo
direction. `dispatch_stage_run` (host->worker) carries the runner stage; while
it waits, the worker initiates `leaven/lm.complete` (worker->host) and
`StageRunEffectHost` answers from the host `HostLm`.
`RunContextGraphEffectHost` is the narrow graph-write callback path: it services
`leaven/proposal.apply` by calling `RunContext::apply_batch`, and services
`leaven/assessment.submit` by calling `RunContext::submit_assessments` after a
host-owned typed lowerer has converted the public payload into `Assessment<P>`.
It services `leaven/evaluation.request` by calling
`RunContext::request_evaluation` after a host-owned typed lowerer has converted
the public payload into an engine `EvaluationRequest`. It services
`leaven/event.emit` by lowering the public write body into
`RunContext::emit(RunEvent::ExternalEventEmitted { ... })`. It exists to prove
the bidirectional stdio callback can cross from a worker request into
RunContext-owned mutation without teaching `leaven-acp` or `leaven-seam-service`
graph internals. Other locked methods still reject unless their own explicit
host lowering is installed.

The candidate prompt template is host-side optimization state. The host renders
it against the case and projects the rendered, model-facing prompt into the
target-free runner `case_input["prompt"]`; the worker reads that and never sees
the case target. Candidate materialization, `graph.query`, the reward vector,
agent, and sandbox are later slices.

## Public Maturity

This crate is a bridge-demo/provenance proof of the bidirectional seam, but
only of the prompt/LM/exact-match path. The LM is a deterministic mock (no
spend, no network); the seam, stage dispatch, and GEPA-shaped accept are real.
It is not proof of the durable `leaven seam serve --stdio` SDK server route,
the reward vector, agent rollout, sandbox, live LM, or `objective != instance`.
It is not re-exported by `leaven`, `leaven::prelude`, or default features as
ordinary app-facing API.

## Proof Anchors

- `tests/example_03_prompt_optimize.rs` runs example 03 end to end over the live
  bidirectional seam: the seed scores zero (the mock LM has no question to
  evaluate), the reflector reads the all-empty feedback and proposes a child that
  surfaces the question, the child scores perfectly through real rollouts, and
  the loop accepts the strict improvement, producing an `Optimized`
  `PromptArtifact`.
- `tests/dispatch_contract.rs` isolates the transport leg: a worker that
  initiates `leaven/lm.complete` during `stage.run` and asserts the stamped
  fingerprint, workers that initiate `leaven/proposal.apply` and
  `leaven/event.emit` and prove the host recorded RunContext-backed graph
  mutations, plus negatives for a non-text stage output and target material
  smuggled into the runner request.
- `tests/graph_effect_contract.rs` isolates graph-effect callbacks that need
  additional typed host lowering. Its `leaven/assessment.submit` and
  `leaven/evaluation.request` proofs keep public JSON parsing at the bridge
  boundary, lower into concrete engine types, call `RunContext`, and assert the
  graph/evidence store where applicable, not just the receipt shape.
- `worker/serve_stage_runner.py` is the runnable `serve_stage` runner worker the
  example spawns. The Python SDK project under `sdk/python`
  stays `NotImplementedError` by its own AGENTS; this runnable worker lives here,
  like `examples/p9_python_acp_gepa_codex/worker`.
- `src/host.rs` and `src/artifact.rs` carry crate-local unit tests for the
  deterministic mock LM, the `lm.complete` request lowering, and prompt
  rendering.

## Verification

- Run `cargo test -p leaven-acp-stage-bridge` after changing the bridge,
  dispatch projection, mock LM, or accept loop.
- Run `cargo test -p leaven-acp -p leaven-public-seam` after changing the
  stage-run dispatch transport method or the stage-run envelope validators.
- Run `cargo test -p leaven --test topology_contract` after changing this
  crate's dependencies or workspace membership.
