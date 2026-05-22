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

1. **Bridge crate skeleton and topology** - done 2026-05-21
   Add `leaven-gepa-agentic-git` with local `AGENTS.md`, manifest entries,
   crate exports, and topology-contract updates. The crate owns only the bridge
   from GEPA reflection to Git-program agentic proposal.

2. **Typed reflection input** - done 2026-05-21
   Add `GitProgramGepaReflectionInput<Part>` carrying
   `GitProgramArtifact + ReflectRequest<Part>`. It must preserve request
   provenance and must not rebuild reflective examples.

3. **Materializer, renderer, parser** - done 2026-05-21
   Compose `GitProgramMaterializer` and `GitProgramReadback` with a renderer
   that gives the agent checked-out repo paths, selected part, examples, and a
   strict patch/bundle/commit output contract. Parser output is a
   `ProposalBatch` with `ProposalEffect::Change`.

4. **GEPA reflector wrapper** - done 2026-05-21
   Add `GepaGitProgramAgenticReflector` mirroring
   `GepaSkillBankAgenticReflector`: resolve parent through `RunContext`, feed
   the materializing `AgenticProposer`, then finalize through
   `RunContext::propose` and `apply_batch`.

5. **No-spend product proof** - done 2026-05-22
   `crates/leaven-gepa-agentic-git/tests/git_reflection.rs` materializes a
   local `GitProgramArtifact`, renders repo-aware GEPA reflection
   instructions, applies an agent-style workspace edit, reads back a typed
   `GitProgramChange`, records the proposal through `RunContext::propose`, and
   admits the child with `apply_batch`. It asserts parent immutability, child
   durable Git contents, proposal provenance, and run-event frontier updates.

6. **P5-shaped fixture** - done 2026-05-22
   The same no-spend proof now wraps the GitProgram child in a tiny
   EvoSkill-shaped `TopKFrontier`: the seed is selected as parent, the child is
   scored and admitted, best-candidate/best-score state is reported, and
   `PopulationUpdated` events are present. This remains a product-backend
   proof, not OfficeQA/SealQA parity.

7. **Docs and verification** - done 2026-05-22
   Update `docs/working-memory/skill-paper-replication.md`, run focused tests,
   run topology if crate inventory changed, run live Firkin only if backend
   code changed, then run `just check`.

## Implementation Note

The full `AgenticProposer<GitProgramArtifact>` wrapper compiles under LLVM, but
the pinned nightly Cranelift dev backend ICEs in rustc's known-panics lint when
the generic agentic future is monomorphized in the integration test. The
default proof therefore exercises the stable product boundary directly:
materializer + renderer + workspace edit + parser + `RunContext::propose` +
`apply_batch` + frontier event reporting. Keep the wrapper code in place; do
not count the LLVM-only wrapper test as the ordinary completion gate until the
toolchain ICE is gone or the generic future shape is smaller.

## Verification Evidence

- `cargo test -p leaven-gepa-agentic-git` passed with 4 default tests.
- `cargo clippy -p leaven-gepa-agentic-git --all-targets -- -D warnings`
  passed.
- LLVM coverage profile ran the full wrapper test gated by `cfg(coverage)`.
- `python3 scripts/coverage-gate.py --line-floor 89.01 --branch-floor 87.31`
  passed with line coverage 89.12% and branch coverage 87.32%.
- `PYTHONDONTWRITEBYTECODE=1 just check` passed warm after the cold run
  rebuilt/listed too slowly for the outer SLA timer. The passing run included
  fmt, line-count lint, clippy, 818 nextest tests, doctests, and coverage.

## Stop Conditions

Stop and ask before provider/cloud/GPU spend, broad OfficeQA/SealQA live runs,
changing the public GEPA API shape beyond the Git bridge need, or replacing
the current Git/Firkin backend design.
