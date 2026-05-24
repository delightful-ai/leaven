## Boundary
`leaven-workspace` is the backend-neutral workspace substrate. It owns
workspace paths, file/command capability vocabulary, factories, lease/view
lifecycles, allocation policy, and typed workspace errors.

It must not know artifacts, surfaces, stores, graph mutation, agent runtimes,
provider protocols, or concrete host/container mechanics.

## Map
- `src/path.rs`: `WorkspacePath`, the normalized relative path type. It rejects
  absolute paths, parent traversal, empty components, and empty parses; the root
  is explicit via `WorkspacePath::root()`.
- `src/workspace.rs` and `src/view.rs`: `Workspace`, `WorkspaceView`,
  `WorkspaceBackend`, and `with_workspace`. Views scope paths; cleanup is
  explicit and consumes the workspace.
- `src/command.rs`: backend-neutral command request/output vocabulary:
  program, args, cwd, env, stdin, limits, user, exit status, captured
  stdout/stderr bytes, captured output-file bytes keyed by workspace path, and
  truncation flags.
- `src/factory.rs`, `src/config.rs`, and `src/policy.rs`: allocation boundary
  and coarse filesystem/network policy vocabulary.
- `src/error.rs`: typed allocation, workspace, cleanup, and path errors.

## Local Helper Stack
- Use `WorkspacePath::new`, `WorkspacePath::root`, and `join` for every
  workspace path crossing this API. Host `PathBuf` belongs inside a concrete
  backend only.
- Use `WorkspaceView` to scope file/command access under a subdirectory; it
  rejects backend paths that escape the scoped prefix.
- Use `with_workspace` for lifecycle-sensitive stages so stage errors and
  cleanup errors are both reported instead of losing one.
- Use `CommandLimits` and `CapturedOutput` for truncation. Large full outputs
  should be stored as blobs/evidence records by the caller, not forced into the
  workspace command result.

## Route Away
- Concrete filesystem, Docker, E2B, Firecracker, git-worktree, Kubernetes, and
  local process behavior belongs in `leaven-workspace-*` backend crates.
- Materializers, renderers, workspace proposal parsers, and agentic stage policy
  belong in `leaven-render`, `leaven-agentic`, or shape-specific adapter crates.
- Agent sessions belong in `leaven-agent*`; they receive an already-materialized
  `WorkspaceView` and must not make this crate understand proposals or
  assessments.
- Graph mutation, cache, budgets, trust, and restore laws belong in
  `leaven-engine`; workspace mutation is not graph mutation.

## Decision Cards
- when: adding a backend-neutral command capability
  do: add the vocabulary here only if multiple backends can honestly support or
    explicitly refuse it
  preserve: unsupported operations being explicit typed errors
  avoid: making local process behavior the default contract for container or
    remote backends
  verify: run `cargo test -p leaven-workspace --test workspace_view`

- when: accepting or returning paths
  do: convert at the backend boundary and keep public APIs on `WorkspacePath`
  preserve: relative, traversal-free, scoped path semantics
  avoid: accepting host paths as a convenience for local tests
  verify: run `cargo test -p leaven-workspace --test workspace_path`

## Local Bait
- `WorkspacePath` is not a host `PathBuf`. `local_mount()` is optional and is
  only for backends that expose a host mount.
- Do not resurrect `WorkspaceRenderer`; the side-effecting path is
  `Materializer` in the rendering/agentic layer.
- Do not rely on `Drop` for normal cleanup. Use `with_workspace` or explicit
  `Workspace::cleanup().await`.
- Default backend operations intentionally return `UnsupportedOperation`; a
  backend that cannot honor `run_command.user`, timeout, executable bits, or
  local mount semantics should refuse them explicitly.
- `WorkspaceConfig` and `FilesystemPolicy`/`NetworkPolicy` are allocation
  vocabulary. Enforcement specifics belong in backends and higher trust policy,
  not in generic artifact or optimizer code.

## Proof Anchors
- `cargo test -p leaven-workspace --test workspace_path` proves public
  workspace paths remain relative, traversal-free, normalized, and explicit
  about root handling.
- `cargo test -p leaven-workspace --test workspace_view` proves scoped views,
  backend delegation, optional local mounts, unsupported-operation defaults,
  and cleanup/error preservation through `with_workspace`.
- `cargo test -p leaven --test topology_contract` proves this neutral crate
  depends only on `leaven-kernel` and backend crates depend inward on it.
