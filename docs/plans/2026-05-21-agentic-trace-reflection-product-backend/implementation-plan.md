# Agentic Trace Reflection Product Backend Plan

Date: 2026-05-21
Status: active execution handoff.
Spec: `docs/specs/agentic_trace_reflection_product_backend.md`.

## Current Fact

The Git/Firkin backend substrate is landed. The next task is not another
workspace backend; it is the GEPA/EvoSkill-shaped product loop over
`GitProgramArtifact`.

## Topology

- Put Git artifact identity changes in `crates/leaven-artifact-git`.
- Keep materialization/readback in `crates/leaven-agentic-git`.
- Add the GEPA reflection bridge as `crates/leaven-gepa-agentic-git` if code
  cannot fit an existing ownership boundary without leaking optimizer concepts.
- Keep optimizer rhythm, frontier, checkpoint, and report state in
  `crates/leaven-gepa`.
- Keep P5 paper pressure in `examples/p5_evoskill_iteration`; do not move
  OfficeQA/SealQA logic into generic crates.

## Execution Slices

1. **Bridge crate skeleton and topology** - started 2026-05-21
   Add `leaven-gepa-agentic-git` with local `AGENTS.md`, manifest entries,
   crate exports, and topology-contract updates. The crate owns only the bridge
   from GEPA reflection to Git-program agentic proposal.

2. **Typed reflection input**
   Add `GitProgramGepaReflectionInput<Part>` carrying
   `GitProgramArtifact + ReflectRequest<Part>`. It must preserve request
   provenance and must not rebuild reflective examples.

3. **Materializer, renderer, parser**
   Compose `GitProgramMaterializer` and `GitProgramReadback` with a renderer
   that gives the agent checked-out repo paths, selected part, examples, and a
   strict patch/bundle/commit output contract. Parser output is a
   `ProposalBatch` with `ProposalEffect::Change`.

4. **GEPA reflector wrapper**
   Add `GepaGitProgramAgenticReflector` mirroring
   `GepaSkillBankAgenticReflector`: resolve parent through `RunContext`, feed
   the materializing `AgenticProposer`, then finalize through
   `RunContext::propose` and `apply_batch`.

5. **No-spend product proof**
   Test a fake agent over a local GitProgram fixture where the child revision
   changes, proposal provenance is preserved, parent stays immutable, and GEPA
   attempt/admission metadata names the child.

6. **P5-shaped fixture**
   Add a tiny EvoSkill-shaped GitProgram run or extend P5 so it exercises the
   bridge, reports actual frontier parent/child/admission truth, and clearly
   remains a product-backend proof rather than OfficeQA/SealQA parity.

7. **Docs and verification**
   Update `docs/working-memory/skill-paper-replication.md`, run focused tests,
   run topology if crate inventory changed, run live Firkin only if backend
   code changed, then run `just check`.

## Stop Conditions

Stop and ask before provider/cloud/GPU spend, broad OfficeQA/SealQA live runs,
changing the public GEPA API shape beyond the Git bridge need, or replacing
the current Git/Firkin backend design.
