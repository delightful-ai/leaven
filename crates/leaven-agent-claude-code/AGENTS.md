## Boundary
This crate is the future Claude Code runtime adapter for the neutral
`leaven-agent` session contract.

Current public names are scaffolding. `ClaudeCodeRuntime` does not yet prove
CLI invocation, protocol parsing, workspace mediation, sandbox policy, or cost
accounting.

## Local Bait
- Generic session/request/result vocabulary belongs in `leaven-agent`; command
  spawning helpers belong in `leaven-agent-command`.
- Claude-specific flags, config, and output parsing stay here and must not leak
  into `leaven-agentic` stage adapters.

## Verification
- `cargo check -p leaven-agent-claude-code` proves only scaffold exports.
- Real behavior needs deterministic CLI-output fixture tests plus explicit
  live/provider-gated tests for actual Claude Code execution.
