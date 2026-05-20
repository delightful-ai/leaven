## Boundary
This example is the SkillReducer paper lane for one tiny live debloating loop.
It is deliberately outside the Cargo workspace so it can advance the paper
replication without introducing Leaven abstractions or touching sibling
workspace state.

## Live Proof
- `bash examples/skillreducer_tiny/scripts/run_tiny_live.sh --preflight`
  writes the no-spend proof contract.
- `LEAVEN_CODEX_LIVE=1 bash examples/skillreducer_tiny/scripts/run_tiny_live.sh --live`
  runs one small skill through Codex with `gpt-5.4-mini`: Stage 1 routing
  description minimization, real-trigger validation, Stage 2 body
  classification/progressive disclosure, faithfulness gate, task-based A/C
  evaluation, and optional feedback promotion.
- The live run spends provider/runtime resources through the Codex CLI. It
  writes generated artifacts under `tmp/skillreducer_tiny/`.

## Local Rules
- Keep SkillReducer-specific logic here until all five paper loops expose a
  repeated boring substrate.
- Do not claim the preflight as replication proof; only the live report with
  Codex logs and generated artifacts counts.
- Preserve the A/C evaluation shape. The original skill is Condition A; the
  compressed core with on-demand references is Condition C.
- Document every paper deviation in `README.md` and the generated report.

