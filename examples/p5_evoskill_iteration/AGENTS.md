## Boundary
This package is the live EvoSkill one-iteration reproduction. It is not the deterministic example lane.

P5 owns the example wiring for a Codex-backed agentic skill loop: source prompt fixtures, skill proposer and builder roles, local workspaces, file evidence, private optimizer checkpoints, resume behavior, and result summaries. Reusable agentic, skill-bank, store, workspace, or graph primitives belong in their owning crates, not in this package.

## Live Proof Model
- `just milestone-p5` runs `LEAVEN_CODEX_LIVE=1 cargo run -p p5_evoskill_iteration -- --live-codex`; it proves the live Codex CLI path can run baseline evaluation, collect failures, propose a skill change, build the child skill bank, checkpoint each phase, evaluate the child, and write `result_summary.json`.
- This command spends live provider/runtime resources through the Codex CLI. It requires the Codex binary path from `LEAVEN_CODEX_BIN` or `$HOME/.bun/bin/codex`, and the runtime currently bypasses sandbox/approval prompts inside the example.
- The run output belongs under `tmp/p5_evoskill_iteration/live-cli` by default, with evidence, run-store checkpoints, preflight checkpoints, workspaces, and summaries treated as generated artifacts.
- Treat this as a live agentic skill-reproduction gate, not a cheap deterministic coverage lane. It is stronger than fake-runtime unit tests for the Codex CLI path, but it still proves this example's one-iteration EvoSkill wiring rather than the ordinary Layer 1 builder surface.

## Local Rules
- Keep `LEAVEN_CODEX_LIVE=1` plus `--live-codex` as the intentional live gate. Do not add a fake deterministic fallback that claims to prove the EvoSkill reproduction.
- Preserve the phase checkpoint enum and resume checks when changing the loop;
  checkpoint state is part of the acceptance surface, not logging. Frontier
  membership, selected parent, and parent-selector cursor are private
  optimizer state and must survive resume.
- Keep train and validation partitions distinct. Baseline and child validation prove selection behavior; train failures drive proposal feedback.
- Preserve the skill-bank mount contract in role instructions: active candidate skills live under `.agents/skills`, while the meta skills are mounted under `.claude/skills` for the live reproduction.
- If a local test touches only pure helper behavior, name it as helper coverage. Do not present it as proof that the live EvoSkill path works.
- Preserve the preflight report. It is the cheap proof that artifact, workload,
  runtime identity, output contract, cache policy, store/checkpoint writes,
  presenter, and scorer are coherent before live provider spend starts.
- Keep proposal and skill-builder roles separate. The proposer emits the JSON
  `SkillProposal`; the builder mutates `.agents/skills` and the parser reads
  the workspace back into `SkillBankChange`.

## Bait
- `cargo test -p p5_evoskill_iteration` only exercises helper-level deterministic assertions. It does not prove Codex execution, workspaces, checkpoints, proposal repair, or live evidence persistence.
- Do not move provider-specific Codex CLI policy into `leaven-agent`, `leaven-agentic`, or skill-bank crates. This package may configure the live reproduction; provider-family behavior belongs under the agent Codex crates.
- `examples/p5_evoskill_iteration/src/codex.rs` currently accepts developer
  instructions but does not store them in `CodexCliConfig`; role prompts are
  rendered into task instructions elsewhere. Do not assume provider-level
  developer-instruction support from this example.
- The example deliberately bypasses Codex sandbox/approval prompts. Keep that
  visible in this package; do not generalize the bypass into provider defaults.
