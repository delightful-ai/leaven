## Boundary
This crate owns the `leaven` binary entry: operator-facing diagnostics, the
durable `seam serve --stdio` public-seam server entry, and the legacy
`serve --stdio` bridge-demo process shell.

It may compose public library crates to render doctor output, inspect local
configuration, simulate stage handoffs, run the public seam over stdio, and run
the bridge-demo engine-client loop. It owns command parsing, process exit
behavior, and stdout discipline for protocol commands. It must not own public
schemas, method dispatch law, transport demux, optimizer accept loops, optimizer
strategy state, provider protocols, or a public API facade.

`seam serve --stdio` delegates public-seam validation/dispatch to
`leaven-seam-runtime` and inherited stdio serving to `leaven-seam-stdio`.
`serve --stdio --plan --out` delegates the bidirectional bridge loop to
`leaven-acp` (`AcpStdioInheritedSession`) and the tiny GEPA-shaped accept loop to
`leaven-acp-stage-bridge` (`optimize_prompt`).

## Map
- `src/main.rs` owns command parsing and process exit behavior.
- `src/doctor.rs` owns doctor commands and render-only diagnostics.
- `src/fixture.rs` owns small deterministic doctor fixtures.
- `src/seam.rs` owns the `seam serve --stdio` command: it loads the locked
  public-seam package, builds a transport-neutral `SeamRuntime`, and serves it
  over inherited stdio. This command exposes the seam and validates traffic; it
  does not claim concrete LM/agent/sandbox/graph execution until those services
  are wired behind the runtime.
- `src/serve.rs` owns the `serve --stdio` command: it reads the locked capability
  env the parent injected, builds the canonical locked profile through
  `leaven-public-seam`, binds an `AcpStdioInheritedSession`, and runs the bridge's
  `optimize_prompt` over it with the deterministic host `MockArithmeticLm`. The
  optimize plan (seed + cases + loop config + named reward/reflect) arrives via
  `--plan` and the `Optimized` result is written to `--out`, so stdin/stdout stay
  a pure JSON-RPC seam.

## Legacy Serve Direction
`leaven serve --stdio` is the ACP **client**, not the agent: it runs the GEPA
accept loop, INITIATES `leaven/stage.run` dispatches to its parent, and SERVICES
the parent's `leaven/lm.complete` callbacks via the host LM. The parent (the
Python SDK or a stand-in test agent) is the ACP **agent**: it serves the runner
stage and calls `leaven/lm.complete` back. The only difference from the bridge
example is the spawn direction — the parent spawns this process, so the session
runs the client loop over inherited stdio with no child spawn. The LM is a
deterministic mock (no spend, no network, no agent, no sandbox); the seam,
dispatch, and accept loop are real. This is the first SDK-shaped product-proof of
the bidirectional seam, not proof of the reward vector, agent rollout, sandbox, or
a live LM.

## Decision Cards
- when: adding a doctor check
  do: keep it deterministic, side-effect free by default, and explicit about
  whether it is inspecting, rendering, or executing
  preserve: operator truth over marketing text; a doctor command must say what
  it did and what it did not prove
  avoid: running live providers, editing workspaces, or mutating run state from
  a command named `doctor`
  verify: run `cargo test -p leaven-cli`

- when: changing the `serve --stdio` command
  do: keep stdout reserved for the JSON-RPC seam (diagnostics to stderr, result
  to `--out`), keep the demux in `leaven-acp` and the accept loop in
  `leaven-acp-stage-bridge`, and validate the launch contract through
  `leaven-public-seam`
  preserve: the engine-as-client direction and the deterministic mock LM default;
  a live LM is a later explicit opt-in
  avoid: re-implementing the bidirectional demux or the optimizer loop here, or
  emitting non-JSON bytes on stdout
  verify: run `cargo test -p leaven-cli --test serve_stdio_optimize`, then
  `cargo test -p leaven-cli`

- when: changing the `seam serve --stdio` command
  do: keep stdout reserved for line-delimited JSON-RPC, delegate validation and
  dispatch to `leaven-seam-runtime`, and keep stdio mechanics in
  `leaven-seam-stdio`
  preserve: this is the durable public seam server route, not the old
  GEPA-specific bridge demo
  avoid: adding provider/runtime behavior in the CLI command itself or emitting
  diagnostics on stdout
  verify: run `cargo test -p leaven-cli` plus `cargo test -p leaven-seam-stdio`
