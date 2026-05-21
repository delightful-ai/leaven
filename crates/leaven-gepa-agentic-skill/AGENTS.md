## Boundary
This crate owns the integration adapter between GEPA reflection and skill-bank
agentic workspace reflection stages.

It may know GEPA reflection requests, the generic `ReflectionWorkspace` flow,
skill-bank projection/readback, and provider-neutral agent runtime contracts.
It must not own GEPA search policy, generic agent runtime behavior, provider
protocols, or skill artifact validation rules.

## Map
- `input.rs` owns the typed bridge from `ReflectRequest + parent SkillBank` into
  `SkillBankReflectionInput`.
- `skill_reflector.rs` implements `SkillBankReflector: ArtifactReflector`,
  projecting the parent bank into `target/current` and reading the edited tree
  back into `SkillBankChange`.
- `reflector.rs` wraps `ReflectionWorkspace` as a `GepaReflector` and records
  the resulting change through `RunContext::propose` before `apply_batch`.

## Decision Cards
- when: changing the GEPA skill-bank agentic reflection path
  do: keep the data bridge build-once-pass-down from `ReflectRequest`; let the
  artifact reflector expose the current parent artifact; let readback return a
  typed `SkillBankChange`
  preserve: `RunContext::propose` followed by `apply_batch`, `ReflectRequest`
  provenance, and provider-neutral `AgentRuntime`
  avoid: adding provider flags here, deriving new reflective examples inside
  the reflector, or bypassing artifact validation/readback diagnostics
  verify: run `cargo test -p leaven-gepa-agentic-skill`
