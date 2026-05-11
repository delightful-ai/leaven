## Boundary
This crate is the future E2B workspace backend for neutral `leaven-workspace`
capabilities.

Current public names are scaffolding. `E2bWorkspaceFactory` does not yet prove
remote sandbox creation, file sync, command execution, cleanup, auth, or retry
behavior.

## Local Bait
- E2B auth and API details stay here; neutral workspace contracts stay in
  `leaven-workspace`.
- Live remote sandbox tests must be explicit opt-ins with clear spend/credential
  requirements.

## Verification
- `cargo check -p leaven-workspace-e2b` proves only scaffold exports.
- Real behavior needs fixture-backed request mapping tests plus opt-in live
  sandbox lifecycle tests.
