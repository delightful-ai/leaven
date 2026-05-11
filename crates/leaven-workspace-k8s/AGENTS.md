## Boundary
This crate is the future Kubernetes workspace backend for neutral
`leaven-workspace` capabilities.

Current public names are scaffolding. `K8sWorkspaceFactory` does not yet prove
pod/job lifecycle, volume layout, command execution, cleanup, auth, or retry
behavior.

## Local Bait
- Kubernetes API details stay here; neutral command/path/lease semantics stay
  in `leaven-workspace`.
- Cluster credentials and live namespaces must be opt-in and isolated.

## Verification
- `cargo check -p leaven-workspace-k8s` proves only scaffold exports.
- Real behavior needs fixture-backed manifest/request tests plus opt-in live
  cluster lifecycle tests with cleanup assertions.
