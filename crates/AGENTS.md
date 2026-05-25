## Boundary
This subtree is the Rust workspace library surface. Each crate is a knowledge boundary, not just a packaging unit.

Root `AGENTS.md` owns the full routing map. This file adds local crate-family rules for placing code and tests inside the flat `crates/` directory.

## Family Map
- Substrate: `leaven-kernel` owns mechanical IDs, cost, finite numbers, fingerprints, metadata, time, and durable error records. It must not learn artifact or optimizer vocabulary.
- Cold algebra: `leaven-core` owns artifact, proposal, evaluation, evidence marker, preference result, and problem vocabulary. It must not know graph, engine, store, surface, workspace, LM, agent, GEPA, or providers.
- Projection and storage seams: `leaven-surface`, `leaven-store`, and `leaven-workspace` own edit surfaces, persistence capabilities, and workspace leases/commands. Their backend crates depend inward on the capability crate.
- Execution: `leaven-engine` owns `RunGraph`, graph views, `RunContext`, budget ledger, trust/read scopes, stage traits, events, cache, persistence, reports, and engine loop.
- Product surface: `leaven-run` and `leaven` own builder ergonomics and re-exports. They compose lower crates; they are not implementation buckets.
- Evaluation adapters: `leaven-eval-parquet` may depend on format libraries to lower physical files into `leaven-eval` source-row contracts. It stays out of engine execution, product defaults, split policy, and paper-specific semantics.
- Standard vocabulary: `leaven-artifact-*`, `leaven-evidence`,
  `leaven-preference`, `leaven-population`, and `leaven-std` own reusable
  vocabulary and implementations at their boundaries. Placeholder catch-all
  artifact/render crates were removed instead of kept as public reservations.
- Runtime adapters: `leaven-lm*`, `leaven-agent*`, `leaven-agentic*`, and `leaven-workspace-*` keep provider/backend details out of cold and engine crates. Provider crates lower to neutral traits; they do not own optimizer rhythm.
- Public seam: `leaven-public-seam` owns the locked V1 external-language worker
  wire contract, active contract package loading, schema/profile inventory,
  schema fingerprints, matrix harness data, and deferred-marker enforcement. It
  must not absorb worker runtime, provider lowering, graph mutation, or
  generated-struct-only proof.
- ACP transport: `leaven-acp` owns the hot stdio process/session transport for
  the locked public seam. It starts external workers and carries JSON-RPC over
  stdin/stdout, while delegating Leaven method/result truth back to
  `leaven-public-seam`. It must not become an MCP bridge, provider runtime, or
  graph mutation layer.
- Optimizers: `leaven-gepa` owns strategy state and search rhythm today. Future
  MIPRO, TextGrad, and trace optimizers should return as behavior-bearing crates
  with local tests, not public reservation crates.
- Edge/domain adapters should return as behavior-bearing crates with local
  tests. Placeholder CUDA/Python adapter crates were removed instead of kept as
  public reservations.
- DSRS interop is not a current workspace crate. Do not treat historical DSRS notes as an edge-adapter precedent until a crate returns with a manifest, topology coverage, tests, and a local boundary file that says what it owns.
- Derive macros are not a current workspace crate. Do not add placeholder macro
  crates; reintroduce `leaven-derive` only with real codegen, UI pass/fail
  fixtures, trait-contract tests, topology coverage, and local ownership docs.

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
- The ACP transport leaf for the locked public seam is `leaven-acp`, a hot
  agent/worker transport adapter, not a public-seam wire-contract bucket. It
  may migrate to the official `agentclientprotocol/rust-sdk` for stdio JSON-RPC
  and process/session mechanics after external dependency approval, but Leaven
  method/result authority stays in `leaven-public-seam`, graph mutation stays
  in `leaven-engine` through `RunContext`, and MCP-over-ACP remains out of V1.
- Do not add placeholder artifact/optimizer/render/domain leaves as workspace
  members. Add the crate only when its first public names have behavior-bearing
  tests and local ownership guidance.

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
