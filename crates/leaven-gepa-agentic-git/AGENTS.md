## Boundary

This crate owns the integration adapter between GEPA reflection and
Git-program agentic proposal stages.

It may know GEPA reflection requests, generic agentic proposer flow,
`GitProgramArtifact`, and Git-program materialization/readback. It must not
own GEPA search policy, generic agent runtime behavior, provider protocols,
Git artifact identity law, Firkin product-pod mechanics, scoring policy, or
frontier admission.

## Status

Explicit scaffold. The crate exists so the product-backend bridge has a real
topology home before behavior is added. It must not be exported as an ordinary
product route until it has a deterministic GEPA GitProgram reflection test that
materializes a parent repo, reads back a typed `GitProgramChange`, and applies
through `RunContext::propose` plus `apply_batch`.

## Map

- `input.rs` will own the typed bridge from `ReflectRequest + parent
  GitProgramArtifact` into an agentic proposal input.
- `materializer.rs` will compose `GitProgramMaterializer` with GEPA reflection
  context files.
- `renderer.rs` will render provider-neutral instructions for editing checked
  out Git program repos.
- `parser.rs` will read final workspace state, patches, or bundles back into a
  proposal batch.
- `reflector.rs` will wrap `AgenticProposer` as a `GepaReflector`.

## Decision Cards

- when: adding the first behavior
  do: mirror the `leaven-gepa-agentic-skill` ownership pattern, keep the
  `ReflectRequest` build-once-pass-down law, and make the parser the only
  readback path into graph proposals
  preserve: `RunContext::propose` followed by `apply_batch`, typed
  `GitProgramChange` readback, and provider-neutral `AgentRuntime`
  avoid: adding provider flags, deriving new reflective examples inside the
  reflector, exposing graph mutation helpers, or treating workspace commits as
  admitted candidates
  verify: run `cargo test -p leaven-gepa-agentic-git` and
  `cargo test -p leaven --test topology_contract`
