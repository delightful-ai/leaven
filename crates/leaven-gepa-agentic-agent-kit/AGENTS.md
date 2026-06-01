## Boundary
This crate owns the deterministic GEPA bridge smoke for repo-backed AgentKit
reflection. It composes AgentKit Codex-profile projection with typed
`GitProgramChange` proposal construction so the first-class Codex kit route has
an executable, provider-free proof.

It does not call Codex, own live-provider auth, execute hooks, run Git
commands, or decide GEPA search policy. Live Codex execution belongs behind an
ignored test gate; Git checkout/readback mechanics remain in `leaven-agentic-git`
and `leaven-gepa-agentic-git`.

The deterministic smoke is not a live Codex conformance proof. Treat this route
as an experimental Codex projection path until the ignored live gate proves the
target Codex surface consumes the projected instructions and skills.

## Routing
- Put deterministic projection/change construction in `src/reflector.rs`.
- Keep `src/lib.rs` maps-only.
- Keep live Codex proof ignored and opt-in.

## Local Bait
- `system_prompt.md` is targetable separately from `AGENTS.md`; do not collapse
  both into one docs slot.
- `hooks/` declarations are scaffold only. This crate may prove they are
  ignored; it must not execute them or make them part of the default Codex
  route.
- Codex CLI/app-server flags belong in provider crates, not this GEPA bridge.

## Proof Anchors
- `cargo test -p leaven-gepa-agentic-agent-kit --test codex_agent_kit_reflection`
  proves deterministic AgentKit projection, hook scaffold refusal, typed
  `GitProgramChange` construction, and graph application through
  `RunContext::propose` plus `apply_batch`.
- `cargo test -p leaven-gepa-agentic-agent-kit` includes the ignored live gate
  without spending provider calls.
- `LEAVEN_CODEX_LIVE=1 cargo test -p leaven-gepa-agentic-agent-kit --test live_codex_agent_kit -- --ignored`
  runs the opt-in local Codex proof. It requires local Codex auth, may spend
  provider budget, and must prove live consumption of the projected
  `system_prompt.md`, `AGENTS.md`, and `.agents/skills` surfaces before
  promoting this path to an ordinary default product route for that Codex
  surface.
- `cargo test -p leaven --test topology_contract` proves this bridge stays free
  of Codex provider protocol dependencies.
