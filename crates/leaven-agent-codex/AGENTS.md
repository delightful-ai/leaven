## Boundary
This crate is the Codex provider-family facade. It owns optional re-export
ergonomics for concrete Codex runtime crates and intentionally does not define
a universal `CodexRuntime`.

## Map
- Feature `cli` explicitly re-exports the current `leaven-agent-codex-cli`
  runtime/config/parser names.
- Feature `app-server` explicitly re-exports the current
  `leaven-agent-codex-app-server` config/error/runtime/transport names.
- Feature `stdio` is an app-server connector feature, not the default
  backend-neutral Codex path.

## Route Away
- Backend-neutral Codex CLI execution belongs in `leaven-agent-codex-cli`.
- Codex app-server protocol, connectors, transport, and transcript
  normalization belong in `leaven-agent-codex-app-server`.
- Provider-neutral runtime vocabulary belongs in `leaven-agent`.
- Skill materialization, proposal parsing, and EvoSkill-shaped stage wiring
  belong in `leaven-agentic-skill`, `leaven-artifact-skill`, or examples.

## Proof Anchors
- `crates/leaven-agent-codex/src/lib.rs` should remain a feature-gated map of
  explicit re-exports only. Do not use wildcard facade re-exports; new leaf
  exports need deliberate maturity review before flowing through this facade.
- `cargo check -p leaven-agent-codex --no-default-features` proves the facade
  does not pull provider dependencies by default.
- `cargo check -p leaven-agent-codex --features cli` proves CLI re-export
  wiring.
- `cargo check -p leaven-agent-codex --features app-server` proves app-server
  re-export wiring.
- `cargo check -p leaven-agent-codex --features stdio` proves the stdio feature
  selects the app-server connector route without making it default behavior.
- `cargo test -p leaven --test topology_contract` proves Codex app-server
  protocol crates stay leaf-only and the umbrella crate does not expose Codex by
  default.

## Decision Cards
- when: adding a Codex provider surface
  do: add or re-export a concrete leaf behind an explicit feature
  preserve: this crate as provider-family import ergonomics, not a runtime implementation
  avoid: defining a shared `CodexRuntime`, shared protocol structs, provider config structs, or agentic parsers here
  verify: run `cargo check -p leaven-agent-codex --no-default-features` plus the feature-specific `cargo check -p leaven-agent-codex --features <feature>`

- when: exposing app-server through the facade
  do: keep it opt-in and keep protocol dependencies gated in `leaven-agent-codex-app-server`
  preserve: topology's leaf-only Codex protocol boundary
  avoid: making app-server or stdio part of default Codex behavior just because the facade can name it
  verify: run `cargo check -p leaven-agent-codex --features app-server`, `cargo check -p leaven-agent-codex --features stdio`, and `cargo test -p leaven --test topology_contract`

## Local Bait
- Do not add shared Codex protocol structs here because both Codex CLI and
  Codex app-server are "Codex". Their operational semantics differ; keep the
  concrete lowering in the concrete runtime leaf.
- Feature names are import promises. `cli` means backend-neutral `codex exec`
  adapter; `app-server` means protocol leaf; `stdio` means local-mount
  connector. Do not blur those into a generic Codex mode.
