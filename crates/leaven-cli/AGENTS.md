## Boundary
This crate owns the `leaven` binary entry: operator-facing diagnostics and the
`serve --stdio` process shell that runs the engine as the ACP client over the
process's own inherited stdio (the wire the Python SDK spawns).

It may compose public library crates to render doctor output, inspect local
configuration, simulate stage handoffs, and run the engine-client serve loop. It
owns command parsing, the launch contract (locked capability env in, result file
out, stdout reserved for JSON-RPC), and process exit behavior. It must not own
the transport demux, the optimizer accept loop, the wire contract, optimizer
strategy state, provider protocols, or a public API facade: `serve` delegates the
bidirectional client loop to `leaven-acp` (`AcpStdioInheritedSession`) and the
tiny GEPA-shaped accept loop to `leaven-acp-stage-bridge` (`optimize_prompt`).

## Map
- `src/main.rs` owns command parsing and process exit behavior.
- `src/doctor.rs` owns doctor commands and render-only diagnostics.
- `src/fixture.rs` owns small deterministic doctor fixtures.
- `src/serve.rs` owns the `serve --stdio` command: it reads the locked capability
  env the parent injected, builds the canonical locked profile through
  `leaven-public-seam`, binds an `AcpStdioInheritedSession`, and runs the bridge's
  `optimize_prompt` over it with the deterministic host `MockArithmeticLm`. The
  optimize plan (seed + cases + loop config + named reward/reflect) arrives via
  `--plan` and the `Optimized` result is written to `--out`, so stdin/stdout stay
  a pure JSON-RPC seam.

## Serve Direction (the crux)
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
