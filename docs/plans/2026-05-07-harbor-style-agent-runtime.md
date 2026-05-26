# Tombstone: Harbor-Style Agent Runtime Implementation Plan

Status: landed and superseded.

This file used to be the implementation plan for command-backed agent runtimes.
It is intentionally no longer a source of truth.

Current owners:

- `docs/specs/agentic_stage_runtime.md` owns the stage/runtime boundary and the
  command-backed provider path.
- `docs/specs/codex_cli_agent_runtime.md` owns the backend-neutral Codex CLI
  adapter contract.
- `docs/specs/codex_app_server_agent_runtime.md` owns the app-server adapter
  contract and its local-mount limitation.
- `crates/leaven-agent-command` owns the reusable command-backed runtime
  substrate.
- `crates/leaven-agent-codex-cli` owns the Codex CLI provider leaf.

Keep future behavior in the specs, crate contracts, and executable tests above.
Do not revive this dated task list as product law.
