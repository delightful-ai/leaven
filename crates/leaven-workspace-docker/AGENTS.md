## Boundary
This crate is the future Docker workspace backend for neutral
`leaven-workspace` capabilities.

Current public names are scaffolding. `DockerWorkspaceFactory` does not yet
prove image selection, mount layout, command execution, cleanup, or sandbox
policy.

## Local Bait
- Do not change neutral `WorkspacePath`, lease, command, or cleanup contracts
  to match Docker. Adapt Docker to the neutral contract here.
- Docker socket access is a host side effect; keep live tests opt-in.

## Verification
- `cargo check -p leaven-workspace-docker` proves only scaffold exports.
- Real behavior needs deterministic factory/config tests and opt-in live Docker
  lifecycle tests that prove cleanup after failure.
