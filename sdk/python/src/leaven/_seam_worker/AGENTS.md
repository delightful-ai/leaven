## Boundary

`leaven._seam_worker` owns the private Python subprocess worker used by
`leaven-seam-service` command-runner stage dispatch.

It loads checked-in SDK `RegisteredStage` objects, receives one locked
`leaven/stage.run` JSON-RPC request on stdin, runs the selected Python runner
or proposer stage, can issue nested `leaven/lm.complete`, `leaven/agent.run`,
and `leaven/proposal.submit_batch` callback requests over the same pipe while
that stage is running, and writes one JSON-RPC response on stdout.

It must not own optimizer strategy, public SDK composition, Rust graph
mutation, service configuration, provider adapters, or transport validation.
The Rust public seam server remains the caller and validator; this package is
only the Python worker process implementation.

## Public Dependencies

- Public stage authoring values: `RegisteredStage`, `PromptArtifact`,
  `InputCaseView`, `ProposalBatch`, `RolloutContext`, and `ProposeContext`.
- The locked runner/proposer `leaven/stage.run` request/result wire shape.

## Private Dependencies

- `leaven._stage_runtime` callback-backed rollout context.
- Sibling modules in this package only.
- No legacy bridge-demo dependency: `leaven._serve` has been removed and must
  not be reintroduced as a parallel worker path.

## Map

- `target.py`: serializable command target for a registered stage.
- `loader.py`: import a stage module/file and resolve a `RegisteredStage`.
- `protocol.py`: one-request JSON-RPC read/write and error envelopes.
- `context.py`: JSON-RPC callback context for the current worker slice,
  including default LM model projection into nested `leaven/lm.complete`.
- `callbacks.py`: private capture of opaque receipts returned by nested
  effect callbacks while one stage invocation is active.
- `runner.py`: runner-stage payload projection and result construction.
- `proposer.py`: proposer-stage payload projection, user stage execution, and
  proposal-submit callback result construction.
- `main.py`: `python -m leaven._seam_worker` entrypoint.
