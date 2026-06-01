## Boundary
This crate owns the repo-backed AgentKit artifact contract: the Leaven-facing
`manifest.toml`, validated kit-relative slot paths, scaffold-only hook
declarations, optional harness declaration, and provider profile records such
as the first Codex projection profile.

AgentKit identity stays the underlying `GitRepoArtifact` or
`GitProgramArtifact` revision. This crate validates the semantic view over that
repo subtree; it does not execute Git, materialize workspaces, run Codex, parse
Codex protocols, or decide optimizer strategy.

## Routing
- Put manifest parsing, profile defaults, and behavior-bearing slot validation
  in `src/manifest.rs`.
- Put portable kit-relative path validation in `src/path.rs`.
- Put provider-neutral profile vocabulary in `src/profile.rs`.
- Reuse `leaven-artifact-skill` for skill-bank semantics. Do not create a
  second `SKILL.md` parser here.

## Local Bait
- `.agents/skills` is a Codex projection mount, not AgentKit identity.
- `hooks/` is scaffold only in this slice. Adding hook execution semantics
  requires a typed law, tests, and an owning runtime/materializer crate.
- `system_prompt.md` and `AGENTS.md` are separate candidate slots. Do not hide
  system-prompt edits inside provider config.
- Codex CLI flags, app-server config, auth, and provider protocol details
  belong in `leaven-agent-codex-*` or materializer leaves, not here.

## Proof Anchors
- `cargo test -p leaven-artifact-agent-kit --test agent_kit_contract` proves
  manifest parsing, path refusal, Codex profile defaults, and hook scaffold
  classification.
- `cargo test -p leaven --test topology_contract` proves this crate remains an
  artifact contract crate and does not grow agentic/provider dependencies.
