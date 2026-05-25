## Boundary

This crate owns the integration adapter between GEPA reflection and
Git-program agentic proposal stages.

It may know GEPA reflection requests, generic agentic proposer flow,
`GitProgramArtifact`, and Git-program materialization/readback. It must not
own GEPA search policy, generic agent runtime behavior, provider protocols,
Git artifact identity law, Firkin product-pod mechanics, scoring policy, or
frontier admission.

## Status

Behavior-bearing advanced bridge route. The crate has a deterministic GEPA
GitProgram reflection test that materializes a parent repo, reads back a typed
`GitProgramChange`, applies through `RunContext::propose` plus `apply_batch`,
records tiny EvoSkill-shaped frontier admission state, and projects the
reflect-then-propose handoff into the locked public-seam stage payloads. It is
not an ordinary prelude/default-feature product route.

## Map

- `input.rs` owns the typed bridge from `ReflectRequest + parent
  GitProgramArtifact` into an agentic proposal input.
- `materializer.rs` composes `GitProgramMaterializer` with GEPA reflection
  context files.
- `renderer.rs` renders provider-neutral instructions for editing checked
  out Git program repos.
- `parser.rs` reads final workspace state, patches, or bundles back into a
  proposal batch.
- `public_seam_stage.rs` lowers the GEPA Git-program reflect/propose attempt
  into locked V1 stage payloads, handoff receipts, and the proposal submission
  plan that cites the proposer-stage receipt.
- `reflector.rs` wraps `AgenticProposer` as a `GepaReflector`.

## Public Maturity

Crate-root exports for `GitProgramPublicSeamStageContext`,
`GitProgramPublicSeamReflectionResult`, and
`GitProgramPublicSeamStageProjection` are advanced public seam projection
contracts for this bridge crate. They are intentionally not in
`leaven_gepa_agentic_git::prelude`; they do not prove ACP delivery, generic
stage payload receipts across all roles, GEPA search policy, or ordinary
facade/default-feature maturity.

## Decision Cards

- when: adding the first behavior
  do: mirror the `leaven-gepa-agentic-skill` ownership pattern, keep the
  `ReflectRequest` build-once-pass-down law, and make the parser the only
  readback path into graph proposals
  preserve: `RunContext::propose` followed by `apply_batch`, typed
  `GitProgramChange` readback, provider-neutral `AgentRuntime`, and separated
  public-seam reflector/proposer stage payloads when proving the external
  worker route
  avoid: adding provider flags, deriving new reflective examples inside the
  reflector, exposing graph mutation helpers, or treating workspace commits as
  admitted candidates
  verify: run `cargo test -p leaven-gepa-agentic-git` and
  `cargo test -p leaven --test topology_contract`
