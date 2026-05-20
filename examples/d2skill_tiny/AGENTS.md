## Boundary
This example is the D2Skill paper lane for one tiny live paired-rollout skill
bank loop. It is deliberately outside the Cargo workspace so the paper replica
can advance without introducing shared Leaven abstractions.

## Live Proof
- `bash examples/d2skill_tiny/scripts/run_tiny_live.sh --preflight` writes the
  no-spend proof contract.
- `LEAVEN_CODEX_LIVE=1 bash examples/d2skill_tiny/scripts/run_tiny_live.sh --live`
  runs one tiny train task through Codex with `gpt-5.4-mini`: paired baseline
  and skill-injected rollouts, utility/retrieval accounting, reflection-driven
  task/step skill generation, next-iteration skill retrieval, utility update,
  and bounded-bank pruning.
- The live run spends provider/runtime resources through the Codex CLI. It
  writes generated artifacts under `tmp/d2skill_tiny/`.

## Local Rules
- Keep D2Skill-specific rollout, retrieval, reflection, utility, and pruning
  logic here until the five paper replicas expose a repeated substrate.
- Preserve paired baseline vs skill-injected rollouts. A single skill-only
  agent call does not prove D2Skill's hindsight utility loop.
- Preserve task-skill and step-skill separation; collapsing them into one
  generic memory misses the paper's dual-granularity point.
- Do not claim the preflight as replication proof. Only the live report with
  Codex logs and generated artifacts counts.
