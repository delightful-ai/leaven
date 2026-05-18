## Boundary
This crate owns the umbrella import experience: curated re-exports, optional
feature gates, preludes, and cross-crate topology/end-to-end contract tests.

It is not an implementation crate. Any behavior added here is suspect until it
has failed to find a more specific owning crate.

## Public Route Model
The umbrella routes every individually re-exported symbol through exactly one
audience-named module. Routes are chosen by **who has a named job**, not by
sophistication tier.

- `prelude` (`leaven::prelude`): ordinary users who define an
  `OptimizationProblem`, call `optimize`, write a scorer, and implement
  `Artifact`/`EditSurface`. ~25 ordinary product types. Nothing else.
- `extend` (`leaven::extend`): users implementing a *piece of the machine* --
  a custom optimizer, proposer, selector, gate, evaluator, materializer,
  store, or LM/agent provider. Engine stage traits, contexts, trust/scope,
  cold algebra. It is not a dumping bag: every symbol carries a `///` doc line
  naming the concrete consumer.
- `plumbing` (`#[doc(hidden)] leaven::plumbing`): public *only* so a sibling
  crate or a contract test can reach it, never for an external caller. No
  stability promise.

`lib.rs` is maps-only: route modules, crate aliases (`pub use leaven_x as x`),
and feature-gated module re-exports. It must carry **no loose
`pub use ...::SomeType;`** at the crate root -- the route module is the
classification. Standard implementations stay behind crate aliases
(`leaven::stdlib`, `leaven::gepa`), never the prelude; do not re-add
`pub use leaven_*::prelude::*` transitive wildcards.

`tests/public_surface_contract.rs` is the enforcement mechanism. It holds a
checked `SURFACE` registry of `(symbol, Route, reason)` and text-parses the
route modules. It fails when a re-exported symbol is unclassified, when a
route module re-exports a symbol with the wrong route, when an `Extend` or
`Plumbing` entry has an empty `reason`, or when `lib.rs` re-exports an
individual symbol. If you cannot name a consumer for an `extend`/`plumbing`
symbol, make it `pub(crate)` instead of routing it.

## Route Here
- Public import shape belongs here: crate aliases, the three route modules,
  and feature-gated module facades.
- Cross-crate contract tests belong here when they prove workspace topology,
  dependency edges, feature/import shape, or public workflows that intentionally
  span multiple crates.
- Feature wiring belongs here when the question is "what should the umbrella
  expose?" not "how does this subsystem work?"
- Public maturity gates belong here when they decide whether a name is safe for
  default-facing users: ordinary prelude membership, default features, scaffold
  allowlists, and tests that fail on production-looking placeholders.

## Route Away
- Runtime logic, helper functions, domain behavior, provider lowering, store
  backends, optimizer strategies, and graph shortcuts belong in the owning
  crate, then may be re-exported here if the import experience calls for it.
- Codex provider-family implementation stays in `leaven-agent-codex*`. This
  crate must not expose Codex provider features until import-experience design
  names that surface.
- Standard reusable behavior belongs in `leaven-std` or the concrete standard
  vocabulary crate, not in the umbrella.

## Proof Anchors
- `src/lib.rs`, `src/prelude.rs`, `src/extend.rs`, and `src/plumbing.rs` are
  the implementation surface: `lib.rs` is a map of aliases and route modules;
  the three route modules are curated re-export lists.
- `tests/public_surface_contract.rs` proves every routed symbol is classified
  with a route and a consumer reason, and that `lib.rs` routes no loose
  individual symbol.
- `tests/topology_contract.rs` proves workspace member inventory, crate
  dependency edges, cold-core leak checks, and Codex app-server protocol
  quarantine.
- `tests/scalar_keep_best.rs`, `tests/pairwise_tournament.rs`, and
  `tests/gepa_parity.rs` prove selected public workflows through the umbrella
  import surface.
- `cargo test -p leaven --test topology_contract` proves manifest/topology and
  quarantine changes.
- `cargo test -p leaven --test public_surface_contract` proves the route
  classification after any `lib.rs`/route-module change.
- `cargo nextest run -p leaven` proves the umbrella import and cross-crate
  workflow contracts.

## Local Bait
- A missing re-export is not proof the behavior belongs here. First add or fix
  the owning crate API, then decide whether the umbrella should expose it.
- Optional dependencies are import promises. Adding one here widens the product
  surface and should be paired with topology tests and feature-gate intent.
- Default features and `prelude` exports are ordinary-user promises. Do not
  expose compile-error derives, placeholder providers/backends, empty standard
  names, fixed GEPA fixtures, or engine-author internals as ordinary imports
  just because the dependency graph allows it.
- `tests/topology_contract.rs` is stronger than stale topology prose for the
  current crate inventory, but it is still a proof anchor, not a dumping ground
  for local crate behavior tests.
- The umbrella surface is routed by audience (`prelude`/`extend`/`plumbing`),
  not by a sophistication split. There is no `prelude::advanced` module:
  engine-author and component-author names (`RunContext`, `RunGraphView`,
  `TrustPolicy`, `EvaluationRequest`, `Proposer`, `Evaluator`, stage traits)
  live in `leaven::extend`; identity/finite-number/error internals live in
  `leaven::plumbing`. Moving a symbol between routes is a public-surface
  change: update `SURFACE` in `public_surface_contract.rs` in the same edit.
- GEPA's fixture-shaped names live under explicit test-support routing (for
  example `leaven::gepa::test_support::FixedSurfaceEdit`) and intentionally do
  not appear in any route module or the ordinary `leaven::gepa` root. A topology
  pass can prove the gepa edge is allowed; only a public-maturity pass proves a
  fixture is honest for ordinary users.
- Every change to `src/lib.rs` or a route module must keep the
  `public_surface_contract.rs` `SURFACE` registry in sync: a new re-export
  needs a route and (for `extend`/`plumbing`) a consumer reason, and a removed
  re-export needs its registry entry deleted. `Cargo.toml` feature changes
  still name the maturity route they touch (feature-gated facade, test
  support, or explicit scaffold); optional provider/backend features are
  public promises, not scaffold markers.

## Decision Cards
- when: adding, removing, or re-routing a re-exported umbrella symbol
  do: route it through exactly one of `prelude`/`extend`/`plumbing`, then add or update its `SURFACE` row in `public_surface_contract.rs` with route and (for extend/plumbing) a consumer reason
  preserve: `prelude` as ordinary-only, `extend` symbols each justified by a named consumer, `lib.rs` as maps-only with no loose individual `pub use`
  avoid: re-adding `pub use ...::prelude::*` transitive wildcards, or routing an `extend`/`plumbing` symbol you cannot name a consumer for (make it `pub(crate)` instead)
  verify: run `cargo test -p leaven --test public_surface_contract` and `cargo nextest run -p leaven`

- when: adding or renaming a feature
  do: prove the feature exposes a behavior-bearing adapter/facade or name it as scaffold/experimental outside defaults
  preserve: default features as ordinary-useful and non-scaffold
  avoid: exposing placeholder provider/backend crates, compile-error derives, or inert standard names under product-looking feature names
  verify: run the feature-specific `cargo check -p leaven --features <feature>` plus the owning crate's focused test; add a maturity/ledger row when the feature changes public import routes

- when: adding a cross-crate workflow test
  do: state whether it is product-proof, mechanics-smoke, or proxy-demo in the test/example docs or nearest `AGENTS.md`
  preserve: topology proof vs public maturity as separate claims
  avoid: letting `gepa_parity`, P8, or coverage runs certify fixed-edit reflection, placeholder providers, or advanced cache wrappers as ordinary product behavior
  verify: run the named workflow test plus the owning crate's narrower gate
