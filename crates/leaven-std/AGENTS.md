## Boundary
This crate is a curated standard-library facade over reusable implementations. It re-exports artifacts, surfaces, evidence, preferences, populations, and render helpers under stable modules and optional feature gates.

It is not an implementation bucket. New behavior belongs in the owning crate first, then may be re-exported here when it is a standard piece.

## Routing
- `artifacts` re-exports `leaven-artifacts` plus optional `git`, `jj`, and `skill` artifact crates behind matching features.
- `evidence`, `preferences`, `populations`, `render`, and `surfaces` re-export their owning crates.
- `prelude` should stay a practical import set, not a dumping ground for every public item.
- Product-builder defaults belong in `leaven-run`; umbrella feature composition belongs in `leaven`.

## Current Audit Pressure
- `leaven-std` currently wholesale re-exports mixed crates in named modules.
  The standard prelude is narrower than those modules, but any placeholder
  exported from an owning crate's prelude still becomes standard-library
  looking here.
- Treat this crate as an export ledger hotspot: every public name exposed here
  should be behavior-bearing, explicit scaffold/test support, or removed from
  the curated facade.
- Optional artifact features are import promises. A feature named `skill`, `git`,
  or `jj` must expose a usable artifact crate before it is presented as
  standard behavior; current placeholder artifact crates remain audit pressure.

## Local Helper Stack
- Prefer narrow `pub use` lists once a module mixes mature and scaffold names.
  Whole-crate glob re-exports are acceptable only while every exported name in
  that route has the same maturity category.
- Keep `prelude` smaller than module exports. The prelude is for common standard
  pieces, not for every reusable experiment.
- When a standard piece graduates, first add/prove behavior in the owning crate,
  then add it here with a focused facade check.

## Local Bait
- Do not add wrapper types here to hide ownership. If a type needs behavior, implement it in the owning crate and decide whether this facade should expose it.
- Optional artifact feature names are part of the import experience. Keep `git`, `jj`, and `skill` aligned with the optional dependencies they expose.
- `src/lib.rs` is allowed to contain the facade map because this crate's whole job is an import map; do not copy that exception into behavior crates.
- Do not use `leaven-std` to launder placeholder symbols into ordinary user
  examples. The audit docs explicitly call this out as public-maturity debt.

## Proof Anchors
- `cargo check -p leaven-std --features git,jj,skill` proves the full facade
  feature set resolves. It does not prove that every re-exported name is
  behavior-bearing.
- `cargo test -p leaven --test topology_contract` proves `leaven-std` remains a facade dependency in the expected workspace topology.
- When pruning or widening exports, add/run the public export ledger or compile
  test requested by `reviews/2026-05-11-fuckery-extermination-today/cross-cutting/surface-requirements.md`.
