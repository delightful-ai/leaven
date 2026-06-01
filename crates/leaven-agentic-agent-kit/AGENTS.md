## Boundary
This crate owns provider-profile materialization for repo-backed AgentKit
artifacts. The first profile is Codex workspace projection: read
`system_prompt.md` as provider instruction input, mount agent docs as
`AGENTS.md`, and mount skills under `.agents/skills`.

It does not call Codex, parse Codex session protocols, own Git checkout/readback
logic, or decide optimizer strategy. It consumes an already checked-out
AgentKit subtree and writes a run workspace projection.

## Routing
- Put Codex profile projection in `src/codex.rs`.
- Keep provider flags, auth, app-server protocol records, and CLI invocation in
  `leaven-agent-codex-*`.
- Keep repo revision identity and readback in Git artifact/agentic Git crates.

## Local Bait
- Symlink vs copy is a mount policy and must be recorded in the projection
  report. It is not candidate identity.
- `hooks/` remains ignored scaffold. Do not execute hook declarations here.
- `system_prompt.md` is returned as instruction text; it is not mounted as
  `AGENTS.md` and not hidden in config.

## Proof Anchors
- `cargo test -p leaven-agentic-agent-kit --test codex_profile` proves Codex
  projection paths, mount policy reporting, and symlink fallback behavior.
- `cargo test -p leaven --test topology_contract` proves this crate stays free
  of Codex provider protocol dependencies.
