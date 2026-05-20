## Boundary
This example is the Trace2Skill paper lane for one tiny live trajectory-to-skill
loop. It is deliberately outside the Cargo workspace and separate from
`trace2skill_spreadsheetbench`, which currently owns manifest/run-artifact
mechanics rather than a live model loop.

## Live Proof
- `bash examples/trace2skill_tiny_live/scripts/run_tiny_live.sh --preflight`
  writes the no-spend proof contract.
- `LEAVEN_CODEX_LIVE=1 bash examples/trace2skill_tiny_live/scripts/run_tiny_live.sh --live`
  runs Codex with `gpt-5.4-mini` through Trace2Skill's three stages: trajectory
  generation under a frozen initial skill, independent error/success analyst
  patch proposals, hierarchical consolidation with guardrails, programmatic
  skill update, and replay of the failed task under the evolved skill.
- The live run spends provider/runtime resources through the Codex CLI and
  writes generated artifacts under `tmp/trace2skill_tiny_live/`.

## Local Rules
- Keep Trace2Skill-specific trajectory, analyst, patch, and consolidation logic
  here until all five paper replicas expose repeated substrate.
- Preserve analyst independence: analysts read the frozen initial skill and one
  trajectory, not each other's proposed patches.
- Preserve many-to-one consolidation. Directly editing the final skill from a
  single trajectory is not Trace2Skill.
- Do not claim the preflight as replication proof. Only the live report with
  Codex logs and generated artifacts counts.

