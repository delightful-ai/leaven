## Boundary
This crate owns the integration adapter between GEPA reflection and skill-bank
agentic proposal stages.

It may know GEPA reflection requests, generic agentic proposer flow, skill-bank
materialization/readback, and provider-neutral agent instructions. It must not
own GEPA search policy, generic agent runtime behavior, provider protocols, or
skill artifact validation rules.

## Map
- `input.rs` owns the typed bridge from `ReflectRequest + parent SkillBank` into
  an agentic proposal input.
- `materializer.rs` projects that input's parent `SkillBank` into a workspace.
- `renderer.rs` renders GEPA reflection input into `AgentInstructions`.
- `parser.rs` reads the final skill workspace back into a proposal batch.
- `reflector.rs` wraps `AgenticProposer` as a `GepaReflector`.

## Decision Cards
- when: changing the GEPA skill-bank agentic reflection path
  do: keep the data bridge build-once-pass-down from `ReflectRequest`; let the
  materializer expose the current parent artifact; let the parser be the only
  readback path into graph proposals
  preserve: `RunContext::propose` followed by `apply_batch`, `ReflectRequest`
  provenance, and provider-neutral `AgentRuntime`
  avoid: adding provider flags here, deriving new reflective examples inside
  the reflector, or bypassing `SkillBankWorkspaceProposalParser` semantics
  verify: run `cargo test -p leaven-gepa-agentic-skill`
