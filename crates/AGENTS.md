## Boundary
This subtree is the Rust workspace library surface. Each crate is a knowledge boundary, not just a packaging unit.

Root `AGENTS.md` owns the full routing map. This file adds local crate-family rules for placing code and tests inside the flat `crates/` directory.

## Family Map
- Substrate: `leaven-kernel` owns mechanical IDs, cost, finite numbers, fingerprints, metadata, time, and durable error records. It must not learn artifact or optimizer vocabulary.
- Cold algebra: `leaven-core` owns artifact, proposal, evaluation, evidence marker, preference result, and problem vocabulary. It must not know graph, engine, store, surface, workspace, LM, agent, GEPA, or providers.
- Projection and storage seams: `leaven-surface`, `leaven-store`, and `leaven-workspace` own edit surfaces, persistence capabilities, and workspace leases/commands. Their backend crates depend inward on the capability crate.
- Execution: `leaven-engine` owns `RunGraph`, graph views, `RunContext`, budget ledger, trust/read scopes, stage traits, events, cache, persistence, reports, and engine loop.
- Product surface: `leaven-run` and `leaven` own builder ergonomics and re-exports. They compose lower crates; they are not implementation buckets.
- Standard vocabulary: `leaven-artifacts`, `leaven-artifact-*`, `leaven-evidence`, `leaven-preference`, `leaven-population`, `leaven-render`, and `leaven-std` own reusable vocabulary, implementations, and reserved public names at their boundaries. Leaf files own current maturity status.
- Runtime adapters: `leaven-lm*`, `leaven-agent*`, `leaven-agentic*`, and `leaven-workspace-*` keep provider/backend details out of cold and engine crates. Provider crates lower to neutral traits; they do not own optimizer rhythm.
- Optimizers: `leaven-gepa`, `leaven-mipro`, `leaven-textgrad`, and `leaven-trace` own strategy state and search rhythm when behavior is real. Several are scaffold/reserved today; read leaf maturity warnings before using public names as proof. Do not move optimizer-specific policy into `leaven-engine`.
- Edge/domain adapters: current workspace adapters are `leaven-cuda` and `leaven-python`; they bridge domains without changing the core topology.
- `leaven-dsrs` is a quarantined orphan directory, not a workspace crate. Do not treat it as an edge-adapter precedent until it has a manifest, `src/lib.rs`, topology coverage, and a local boundary file that says what it owns.
- `leaven-derive` is derive macros only. Do not add runtime or adapter dependencies without an explicit derive contract.

## Leaf Activation Rules
Every workspace crate now has a local `AGENTS.md`; read the stacked root, this
file, and the crate-local file before editing. The crate-local files own
scaffold status, proof loops, and bait warnings. This parent keeps only
cross-family rules that apply before you know a leaf's details.

- Store backends implement `leaven-store` traits and must not define graph
  checkpoint schemas, product defaults, workspace behavior, or evidence
  semantics beyond storage capability contracts.
- Workspace backends implement `leaven-workspace` capabilities and must adapt
  backend-specific auth, command, path, lease, and cleanup behavior to the
  neutral workspace contract instead of changing it.
- LM provider leaves lower provider wire/runtime details to `leaven-lm`. They
  must not copy another provider's transport facts, own Leaven response-cache
  policy, or encode GEPA reflection policy.
- Non-scaffold LM provider features must implement the `Lm` trait, expose a real
  constructor/config path, return typed `LmError` failures, and carry at least
  one non-network mapping or law test. A package description that still says
  "skeleton" is stale metadata when the crate has real behavior; it is not a
  license to ignore public-maturity routing.
- Agent runtime leaves keep provider CLI/protocol details local; generic session
  vocabulary stays in `leaven-agent`, command substrate in
  `leaven-agent-command`, and stage parsing in `leaven-agentic`.
- Placeholder artifact/optimizer/render/domain leaves may keep public
  reservation names only when the crate-local file says they are scaffolding.
  Before exposing one through defaults, replace the placeholder with
  behavior-bearing tests or mark the facade as scaffold/experimental.

## Local Rules
- `src/lib.rs` and `src/prelude.rs` are maps only: module declarations, curated re-exports, and optional crate docs. Put behavior in a named owning module.
- Public API is a durable promise. Do not make fields, modules, or helpers public just to satisfy tests.
- Start new modules private. Widen to `pub(crate)` or `pub` only when a real downstream boundary needs it.
- Keep dependency direction visible in `Cargo.toml`. A dependency shortcut that crosses family boundaries must be reflected in specs and topology tests.
- Skeleton crates may be intentionally thin, but once they expose behavior, add focused tests in the owning crate.

## Decision Cards
- when: adding behavior to a scaffold backend, provider, optimizer, domain, or artifact leaf
  do: keep the family rule from this file, then update the local `AGENTS.md` from scaffold quarantine to behavior-bearing guidance
  preserve: inward dependency direction and provider/backend facts staying in their leaf
  avoid: citing a sibling `AGENTS.md` as context for the leaf you are editing, or leaving public placeholder names exposed as ordinary product contracts
  verify: run the leaf's focused test plus `cargo test -p leaven --test topology_contract` if dependencies or features changed

- when: turning a skeleton/reserved crate into a real implementation
  do: add the smallest local boundary doc, focused tests, and any topology/spec update in the same change
  preserve: the parent family map until local behavior proves a sharper rule
  avoid: leaving the coverage matrix or crate-local guidance saying scaffold/reserved after behavior lands
  verify: run the crate's focused test command and the narrow proof named in the new local `AGENTS.md`

- when: moving a type or trait across crates
  do: first name which crate is allowed to know each fact, then update root/crates/local `AGENTS.md` at the same altitude as the new truth
  preserve: cold core below engine/std/workspace/derive and adapter/runtime details outside cold crates
  avoid: dependency shortcuts that only make the current test easier
  verify: run `cargo test -p leaven --test topology_contract` before wider gates

## Verification
- Crate boundary changes: `cargo test -p leaven --test topology_contract`.
- Public API changes in one crate: run that crate's nextest target, for example `cargo nextest run -p leaven-core`.
- Engine/run graph changes: run the focused `leaven-engine` integration test touched by the change, then `just test`.
- Completion gate for behavior: `just check`.
