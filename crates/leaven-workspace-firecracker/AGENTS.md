## Boundary
This crate is the future Firecracker workspace backend for neutral
`leaven-workspace` capabilities.

Current public names are scaffolding. `FirecrackerWorkspaceFactory` does not
yet prove VM image layout, boot lifecycle, command execution, networking,
cleanup, or isolation policy.

## Local Bait
- VM lifecycle and host privileges are backend facts here; do not leak them
  into `leaven-workspace`, engine, or product builders.
- Any live VM proof must be isolated and opt-in, not part of cheap local gates.

## Verification
- `cargo check -p leaven-workspace-firecracker` proves only scaffold exports.
- Real behavior needs deterministic config tests and explicit live lifecycle
  tests for boot, command, teardown, and failure cleanup.
