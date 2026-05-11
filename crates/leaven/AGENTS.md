## Boundary
This crate owns the umbrella import experience: curated re-exports, optional
feature gates, preludes, and cross-crate topology/end-to-end contract tests.

It is not an implementation crate. Any behavior added here is suspect until it
has failed to find a more specific owning crate.

## Route Here
- Public import shape belongs here: top-level crate aliases, curated re-exports,
  feature-gated facades, and `prelude` membership.
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
- `src/lib.rs` and `src/prelude.rs` are the implementation surface: they should
  remain maps of aliases, re-exports, and feature-gated facades.
- `tests/topology_contract.rs` proves workspace member inventory, crate
  dependency edges, cold-core leak checks, and Codex app-server protocol
  quarantine.
- `tests/scalar_keep_best.rs`, `tests/pairwise_tournament.rs`, and
  `tests/gepa_parity.rs` prove selected public workflows through the umbrella
  import surface.
- `cargo test -p leaven --test topology_contract` proves manifest/topology and
  quarantine changes.
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
- Today `src/prelude.rs` exports engine-author names (`RunContext`,
  `RunGraphView`, `TrustPolicy`, `EvaluationRequest`, `Proposer`, `Evaluator`),
  GEPA's prelude, standard names, and LM-cache names behind features. Treat that
  as an audited leak in the ordinary import story, not precedent for adding more
  advanced surfaces to the default prelude.
- The `gepa` default feature currently makes GEPA's fixture-shaped names easier
  to import through `leaven::prelude::*`. A topology pass can prove the edge is
  allowed; only a public-maturity pass proves it is honest for ordinary users.

## Decision Cards
- when: changing `src/prelude.rs` or default features
  do: classify each exported name as ordinary user, GEPA customizer, engine author, LM/runtime, cache-store, or explicit scaffold before adding it
  preserve: a default import experience that exposes behavior-bearing ordinary contracts, not file layout or future-work names
  avoid: adding `pub use ...::prelude::*` for a whole crate unless every exported name is mature at this layer
  verify: run `cargo nextest run -p leaven` and `cargo test -p leaven --test topology_contract`; add an import/export test when the change is about ordinary-vs-advanced visibility

- when: adding a cross-crate workflow test
  do: state whether it is product-proof, mechanics-smoke, or proxy-demo in the test/example docs or nearest `AGENTS.md`
  preserve: topology proof vs public maturity as separate claims
  avoid: letting `gepa_parity`, P8, or coverage runs certify fixed-edit reflection, placeholder providers, or advanced cache wrappers as ordinary product behavior
  verify: run the named workflow test plus the owning crate's narrower gate
