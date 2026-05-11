# Cross-Cutting Fix Priority Map

Status: canonical cross-cutting audit doc.

This orders crate graph, LM/cache, topology, facade, example, and public
maturity fixes. It is not a compatibility plan. Each item is a hard-cut
correction: remove the false public path, implement the real contract, or move
the name into explicit scaffold/test-support scope.

## P0: Install A Public-Maturity Gate Above The Existing Topology Gate

- severity: blocker
- surface: topology contract, crate graph, public exports
- ideal contract: topology is necessary but not sufficient. The corrected
  topology spec says dependency allowlists should be CI-enforced
  (`docs/specs/leaven_v0_2_1b_corrected_crate_topology_lib_rs.md:257-314`),
  and the repo contract says `lib.rs` files are curated maps, not logic or test
  holes (`AGENTS.md:33-42`).
- current implementation: `crates/leaven/tests/topology_contract.rs` validates
  workspace members, manifest and `src/lib.rs` skeleton presence, exact
  dependency edges, one cold-core leak class, and Codex protocol leaf-ness
  (`crates/leaven/tests/topology_contract.rs:420-505`). It does not check public
  maturity.
- blocker/gap: skeletons can satisfy topology while public capability remains
  inert. `crates/leaven-dsrs` is a visible example: it is named in the topology
  spec (`docs/specs/leaven_v0_2_1b_corrected_crate_topology_lib_rs.md:120-136`)
  but is not a workspace member (`Cargo.toml:3-64`) and has no manifest/lib root.
- user impact: future work can add or preserve crate shells and pass topology
  while still failing off-the-shelf optimizer-library use.
- correction direction:
  - add orphan `crates/*` rejection;
  - add skeleton-description rejection outside explicit scaffold allowlists;
  - add public unit-struct deny/allow ledger;
  - add crate-root `pub mod` export ledger;
  - add default-feature maturity checks.
- required proof/tests:
  - `cargo test -p leaven --test topology_contract` fails before the fix on at
    least one seeded maturity fixture or new allowlist test;
  - the final topology contract rejects `crates/leaven-dsrs` unless deleted or
    restored as a real workspace crate;
  - `just check` remains the completion gate.

## P1: Hard-Cut Default `leaven` Imports To Behavior-Bearing Ordinary Contracts

- severity: blocker
- surface: `leaven`, `leaven::prelude`, `leaven-std`, derive, GEPA default
- ideal contract: Tier 1 users should call the short optimize/train/score/GEPA
  path and not learn every internal trait
  (`docs/specs/initial_library.md:451-468`,
  `docs/specs/gepa_public_private_surface.md:20-47`). The umbrella crate should
  be an import experience, not an implementation bucket
  (`crates/leaven/src/lib.rs:1-4`).
- current implementation: default features enable `std`, `derive`, and `gepa`
  (`crates/leaven/Cargo.toml:38-42`); `leaven::prelude` exports engine-author
  names, derive macros, GEPA prelude, std prelude, and LM-cache prelude
  (`crates/leaven/src/prelude.rs:3-49`). Derive macros intentionally expand to
  `compile_error!` (`crates/leaven-derive/src/unimplemented.rs:3-8`).
- blocker/gap: ordinary import paths expose things that are advanced, inert, or
  non-working.
- user impact: a user starting from the advertised umbrella import path gets a
  muddled API and may hit compile-error derives or placeholder standard names.
- correction direction:
  - remove `derive` from defaults until implemented;
  - remove `gepa` from defaults until real reflection exists, or land real
    reflection first;
  - split ordinary prelude from advanced engine-author prelude;
  - keep `leaven-std` out of defaults or prune it to behavior-bearing exports;
  - keep `lm-cache` out of ordinary prelude unless the public story is a runtime
    role policy rather than wrapper stacking.
- required proof/tests:
  - default-feature compile test imports `leaven::prelude::*` and cannot import
    compile-error derives or placeholder provider/backend types;
  - advanced-prelude test proves `RunContext`, `RunGraphView`, and stage traits
    are still reachable intentionally;
  - export ledger documents every default-facing `pub use`.

## P2: Resolve LM/Cache Composition At The Run/Runtime Role Layer

- severity: high
- surface: `leaven-lm`, `leaven-lm-cache`, `leaven-run`, GEPA reflection
- ideal contract: `leaven-lm` is provider-neutral, `leaven-lm-cache` is reusable
  response-cache policy/key/store/wrapper, and GEPA depends on `leaven-lm` but
  not concrete providers or cache stores by default
  (`docs/specs/lm_runtime_and_response_cache.md:33-57`).
- current implementation: `CachedLm<M, C>` and cache policies are real and tested
  (`crates/leaven-lm-cache/src/cached.rs:6-116`,
  `crates/leaven-lm-cache/tests/cache_contract.rs:76-150`), but the ordinary LM
  spec example teaches manual wrapper stacking
  (`docs/specs/lm_runtime_and_response_cache.md:17-28`), and the user called that
  public shape suspicious
  (`reviews/2026-05-11-fuckery-extermination-today/complaints/session-user-messages-for-codex.md:292-299`).
- blocker/gap: there is no canonical place to configure solver LM, reflector LM,
  scorer/judge LM, agent runtime, cache policy, and budget policy by role.
- user impact: AIME live solving bypasses Leaven LM/cache entirely by shelling
  out to Python (`examples/p8_aime_gepa/src/main.rs:293-301`), so the product
  example does not prove provider-neutral LM, OpenAI lowering, response cache, or
  cost accounting.
- correction direction:
  - introduce or designate a run/runtime-role composition root above GEPA;
  - configure cache policy by role;
  - keep `CachedLm` as advanced/test implementation detail;
  - resolve the GEPA spec contradiction by forbidding `leaven-gepa ->
    leaven-lm-cache` in ordinary topology
    (`docs/specs/gepa_optimizer_surface.md:174-197`).
- required proof/tests:
  - product-level test with independent solver and reflector role policies;
  - cache hit returns zero metered cost while preserving stored usage;
  - provider continuation is excluded from response-cache identity;
  - `p8_aime_gepa` live path, when present, uses `OpenAiLm` through `Lm` rather
    than a Python script.

## P3: Stop Publishing Provider/Backend Feature Names For Inert Types

- severity: high
- surface: optional features, provider/backend crates, umbrella re-exports
- ideal contract: optional provider/backend features should expose usable
  integrations or be absent. A feature gate is not a scaffold disclaimer.
- current implementation: the umbrella exposes optional provider/backend features
  (`crates/leaven/Cargo.toml:49-55`) and re-exports LM provider crates
  (`crates/leaven/src/lib.rs:68-75`), while several enabled crates expose only
  inert public structs such as `AnthropicLm`, `LocalLm`, `DockerWorkspaceFactory`,
  `E2bWorkspaceFactory`, `ObjectStore`, and `SqliteStore`
  (`crates/leaven-lm-anthropic/src/client.rs:1`,
  `crates/leaven-lm-local/src/client.rs:1`,
  `crates/leaven-workspace-docker/src/factory.rs:1`,
  `crates/leaven-workspace-e2b/src/factory.rs:1`,
  `crates/leaven-store-object/src/store.rs:1`,
  `crates/leaven-store-sqlite/src/store.rs:1`).
- blocker/gap: the import surface says integrations exist when no trait
  implementation, constructor, typed error, or behavior is present.
- user impact: users spend time enabling features that cannot work, and future
  implementors can mistake reserved names for live architecture.
- correction direction:
  - remove feature/export from `leaven` until capability exists, or implement
    the backend/provider against its owning trait;
  - reserve future shells only behind explicit `scaffold-*` features that are not
    defaults and not used by product examples.
- required proof/tests:
  - every non-scaffold provider feature has a trait-impl compile test and a
    non-network mapping/law test;
  - every non-scaffold workspace/store backend feature has a constructor or
    factory law test;
  - public-maturity gate rejects one-line public unit structs in feature crates.

## P4: Make GEPA Default-Facing Only After Real Reflection And Slot Contracts Land

- severity: blocker
- surface: GEPA imports, LM reflection, public examples
- ideal contract: GEPA customizers swap strategy slots without writing a new
  optimizer (`docs/specs/initial_library.md:470-485`), and `Gepa::builder()` must
  expose surface, population, parent selector, part selector, batch sampler,
  reflector/LM, acceptance, validation, merge, budget/iteration, seed, tracking,
  and split policy (`docs/specs/gepa_optimizer_surface.md:271-304`).
- current implementation: `Gepa` is default-facing via `leaven` defaults and
  prelude (`crates/leaven/Cargo.toml:38-42`,
  `crates/leaven/src/prelude.rs:33-34`), but public GEPA exports include
  `SurfaceProposer`, fixed-edit `ReflectiveMutation`, and placeholder config
  names (`crates/leaven-gepa/src/lib.rs:18-34`,
  `crates/leaven-gepa/src/proposer.rs:21-56`,
  `crates/leaven-gepa/src/optimizer.rs:716-722`).
- blocker/gap: reflection cannot consume assessment IDs, casewise evidence,
  selected part view, attribution evidence, objective/background, or LM input as
  required by the spec (`docs/specs/gepa_optimizer_surface.md:447-483`).
- user impact: examples can claim GEPA while using a canned edit. `p8_aime_gepa`
  does that today (`examples/p8_aime_gepa/src/main.rs:80-99`).
- correction direction:
  - rename/move fixed-edit proposer into test/demo scope;
  - implement a mock-LM reflective proposer over `leaven-lm`;
  - expose real builder slots and reject incomplete/contradictory configs before
    run start;
  - only then restore GEPA to ordinary defaults.
- required proof/tests:
  - `cargo nextest run -p leaven-gepa` covers builder rejection, part selection,
    deterministic sampler, acceptance laws, mock-LM reflection, typed parse
    errors, split policy, and checkpoint state
    (`docs/specs/gepa_optimizer_surface.md:623-637`);
  - product scenario tests prove GEPA through `leaven-run`
    (`docs/specs/gepa_optimizer_surface.md:639-652`).

## P5: Classify Examples And Coverage As Product-Proof, Mechanics-Smoke, Or Proxy-Demo

- severity: high
- surface: examples, coverage gate, acceptance evidence
- ideal contract: examples that prove public maturity must exercise the public
  path they claim. The prototype order exists to surface design problems early,
  not to accumulate green proxy demos
  (`docs/specs/initial_library.md:4638-4683`).
- current implementation: coverage runs all milestone examples including
  `p8_aime_gepa` (`scripts/coverage-gate.py:13-24`). The AIME README says the
  deterministic path proves mechanics and is not evidence of live AIME
  improvement (`examples/p8_aime_gepa/README.md:9-33`).
- blocker/gap: the coverage gate does not distinguish public product proof from
  deterministic mechanics smoke.
- user impact: "green coverage" can hide the fact that no real LM/cache/reflection
  product path was exercised.
- correction direction:
  - add an example classification manifest or script table;
  - keep mechanics/proxy demos in coverage if useful, but prevent them from
    satisfying product maturity gates;
  - require the canonical AIME/GEPA acceptance path to use Leaven LM/cache and
    real reflection.
- required proof/tests:
  - coverage tooling prints or checks example classification;
  - at least one `product-proof` example exercises `leaven-run`, GEPA,
    `leaven-lm`, cache policy, evidence, graph, and result facade end to end.

## P6: Clean Stale Topology And Inventory Truth Without Broad Product Code Changes

- severity: medium-high
- surface: specs, inventory, stale directories
- ideal contract: docs/specs are durable truth, and code/doctrine mismatches
  should be resolved in the same change (`AGENTS.md:88-99`).
- current implementation: `docs/specs/leaven_v0_2_1b_corrected_crate_topology_lib_rs.md`
  names `leaven-dsrs` in layout and dependency sections
  (`docs/specs/leaven_v0_2_1b_corrected_crate_topology_lib_rs.md:120-136`,
  `docs/specs/leaven_v0_2_1b_corrected_crate_topology_lib_rs.md:537-561`),
  while the live workspace omits it (`Cargo.toml:3-64`) and the directory has
  only one-line public structs with no manifest/lib root.
- blocker/gap: stale topology keeps attracting future routing mistakes.
- user impact: DSRS integration can be treated as a current Leaven crate when it
  is neither compiled nor tested.
- correction direction: delete the orphan and remove topology references, or
  hard-cut it back in as a real crate with manifest, lib root, trait impls, tests,
  and topology membership. Do not leave it as a reminder.
- required proof/tests: topology contract must prove there are no unregistered
  `crates/*` directories, or must fail with an explicit deletion-date allowlist.
