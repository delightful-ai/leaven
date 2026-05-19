## Boundary
This crate owns operator-facing Leaven CLI diagnostics.

It may compose public library crates to render doctor output, inspect local
configuration, and simulate stage handoffs. It must not become a hidden runtime
implementation, optimizer strategy home, provider protocol crate, or public API
facade.

## Map
- `src/main.rs` owns command parsing and process exit behavior.
- `src/doctor.rs` owns doctor commands and render-only diagnostics.
- `src/fixture.rs` owns small deterministic doctor fixtures.

## Decision Cards
- when: adding a doctor check
  do: keep it deterministic, side-effect free by default, and explicit about
  whether it is inspecting, rendering, or executing
  preserve: operator truth over marketing text; a doctor command must say what
  it did and what it did not prove
  avoid: running live providers, editing workspaces, or mutating run state from
  a command named `doctor`
  verify: run `cargo test -p leaven-cli`
