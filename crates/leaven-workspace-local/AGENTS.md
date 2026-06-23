## Boundary
`leaven-workspace-local` is the trusted local filesystem/process backend for
the workspace capability crate. It owns tempdir allocation, host-path mapping
inside one local root, local command spawning, local executable bits, and local
cleanup behavior.

It depends inward on `leaven-workspace`; it must not change backend-neutral
path, command, factory, or cleanup contracts.

## Map
- `src/factory.rs`: `LocalWorkspaceFactory` allocates unique roots below either
  the process temp directory or an explicit parent.
- `LocalWorkspaceBackend`: maps `WorkspacePath` to a path under the allocated
  root, implements recursive file listing, read/write, executable toggles, local
  command execution, output truncation, timeout enforcement, and explicit
  cleanup.
- `tests/local_workspace.rs`: the local backend contract for mount visibility,
  unique roots, default temp parent, cleanup tolerance, command cwd/env/stdin,
  truncation, timeout, user refusal, executable toggles, recursive listing, and
  filesystem error mapping.

## Local Helper Stack
- Use `LocalWorkspaceFactory::new(parent)` in tests that need deterministic
  cleanup roots; use `Default`/`temp()` only when the exact parent is irrelevant.
- Commands run through the host process with cwd scoped under the allocated
  root, explicit env/stdin, independent stdout/stderr truncation, and timeout
  polling.
- File listing returns recursive workspace-relative files sorted by
  `WorkspacePath`; do not expose host path order.
- Executable-bit helpers are Unix-backed here and explicit unsupported behavior
  elsewhere. Keep feature/OS differences visible in this backend.
- Host-path mapping refuses existing symlink components before local read,
  write, list, executable-bit, command-cwd, and cleanup operations. Lexical
  `WorkspacePath` validation is necessary but not sufficient for this backend.

## Route Away
- Add or change path/command/factory vocabulary in `leaven-workspace`, then
  update this backend as one implementor.
- Put Docker, E2B, Firecracker, git-worktree, and Kubernetes mechanics in their
  own backend crates. Do not hide local-only assumptions in the neutral trait to
  make this backend easier.
- Put provider CLI setup, Codex app-server layout, transcript parsing, and
  proposal parsing in agent/provider/agentic crates. This backend only runs
  commands and moves bytes inside the workspace root.

## Decision Cards
- when: changing command execution
  do: prove cwd, env, stdin, output truncation, timeout, and user-refusal
    behavior together
  preserve: local backend honesty about trusted host execution
  avoid: weakening the neutral `Command` contract to match a local-only shortcut
  verify: run `cargo test -p leaven-workspace-local --test local_workspace`

- when: changing cleanup or mount behavior
  do: keep cleanup idempotent for already-removed roots and keep local mounts
    optional at the neutral API
  preserve: `Workspace::cleanup().await` as the normal lifecycle path
  avoid: relying on `Drop` or exposing host paths to stage code
  verify: run the local workspace allocation/cleanup tests

## Local Bait
- Local command execution is trusted local development behavior, not the
  sandbox contract for every workspace backend.
- A host mount is guaranteed here but optional in the neutral API. Do not write
  caller code that requires `local_mount()` unless the caller explicitly
  requests this backend.
- `CommandUser` is refused rather than ignored. Keep that pattern for any local
  capability the backend cannot honestly satisfy.
- The backend maps workspace paths under one allocated root; never accept a host
  path from stage code as an escape hatch.
- A passing local command test is not a sandbox proof. Container/remote
  semantics belong in their own backend crates with their own AGENTS guidance.

## Proof Anchors
- `cargo test -p leaven-workspace-local --test local_workspace` proves the
  concrete local backend: allocation roots, mount visibility, cleanup,
  command cwd/env/stdin/limits, user refusal, executable bits, recursive lists,
  and IO error mapping.
- `cargo test -p leaven-workspace --test workspace_view` proves the neutral
  behavior this backend is implementing: scoped paths, optional mounts,
  unsupported-operation semantics, and cleanup error preservation.
- `cargo test -p leaven --test topology_contract` proves this backend depends
  inward on `leaven-workspace` and does not pull engine/store/agent crates into
  local workspace mechanics.
