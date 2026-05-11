# Layer 2 Reflection And Proposal

Status: active findings recorded.

This file audits reflective mutation, proposal generation, LM/agent-backed
reflection, and whether GEPA uses the engine proposer path or duplicates it.

## Findings

### L2-005: GEPA reflection bypasses the engine proposer contract

- severity: blocker
- evidence: `crates/leaven-gepa/src/proposer.rs:6-19`,
  `crates/leaven-gepa/src/optimizer.rs:536-594`,
  `crates/leaven-engine/src/stage/proposer.rs:28`,
  `crates/leaven-engine/src/context/run_context.rs:191-208`,
  `docs/specs/initial_library.md:2174-2233`
- promised behavior: proposal generation is an async stage that can use
  `ProposalContext`, budget, render/materialize context, graph/evidence access,
  and engine proposal finalization.
- actual behavior: GEPA calls a local synchronous `SurfaceProposer` directly,
  then separately records and applies the proposal batch in GEPA code.
- why it matters: GEPA has a parallel proposal path that does not use the
  public engine proposal seam. Reflective mutation cannot be swapped for an LM
  or agent stage through the engine stage contract.
- correction direction: model GEPA mutation as a request to an engine
  `Proposer<P>` or replace `SurfaceProposer` with an async GEPA proposer that
  receives a real proposal context and finalizes through `RunContext::propose`.

### L2-006: `ReflectiveMutation` is a production-looking fixed fixture

- severity: blocker
- evidence: `crates/leaven-gepa/src/proposer.rs:21-47`,
  `crates/leaven-gepa/src/proposer.rs:50-54`,
  `examples/p8_aime_gepa/src/main.rs:91`,
  `docs/specs/gepa_optimizer_surface.md:450-483`
- promised behavior: reflective mutation consumes feedback/traces and proposes
  edits through an LM/agent-capable stage.
- actual behavior: `ReflectiveMutation` stores one edit and returns it every
  time. It ignores artifact, surface, and part. Related config/merge names are
  placeholders.
- why it matters: examples can present a GEPA-looking API while bypassing the
  hard part of GEPA.
- correction direction: rename the fixed edit to a test/demo fixture or remove
  it from the public GEPA surface. Reserve `ReflectiveMutation` for the real
  trace/evidence-aware reflector.

### L2-007: Reflection receives no parent feedback, assessment IDs, or part view

- severity: high
- evidence: `docs/specs/gepa_optimizer_surface.md:460-483`,
  `crates/leaven-gepa/src/optimizer.rs:548-560`,
  `crates/leaven-gepa/src/optimizer.rs:612-620`
- promised behavior: reflector input includes selected parent, selected part,
  part view, assessment IDs, casewise feedback/evidence, attribution, lineage,
  and background.
- actual behavior: GEPA evaluates parent scores, selects a part, then calls
  `propose_edit(&artifact, &surface, &part)`. The reflector does not receive
  assessment IDs, feedback payloads, traces, part view, objective text, or
  rendered context.
- why it matters: a real reflector must fork GEPA or smuggle context through
  artifact/surface state.
- correction direction: define a `ReflectiveMutationRequest` containing parent
  candidate, selected part view, selected evidence refs/payloads or rendered
  views, trace excerpts, lineage, and budget/runtime handles.
