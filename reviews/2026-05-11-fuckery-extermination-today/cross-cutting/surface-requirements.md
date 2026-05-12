# Cross-Cutting Surface Requirements

Status: canonical cross-cutting audit doc.

This is the exact requirement set for facades, default features, topology tests,
public stubs, provider/backend exposure, LM/cache composition, examples, and
public maturity gates. It is not an implementation plan and it is not a
compatibility plan. Leaven should hard-cut false public surfaces.

## 1. Public Maturity Categories

Every exported or proof-bearing name from `crates/*`, examples, or feature gates
must be in exactly one category for each route where it appears. The category is
not just about the symbol; it is about the symbol plus the route. `CachedLm` can
be an advanced public contract through `leaven-lm-cache` and still be forbidden
from the ordinary prelude. A fixed edit proposer can be valid as test support and
still be invalid as `ReflectiveMutation` in a product-proof example.

The maturity ledger row shape is:

```text
path + line
owning crate
symbol or module path
route: crate-root | ordinary-prelude | advanced-prelude | feature | example | test-support
category
behavior proof: test path, law name, product-proof example, or none
decision: keep | move | rename fixture | remove/export-gate | implement
```

Unknown rows fail. `behavior proof: none` is allowed only for explicit scaffold
features and private fixtures, and those routes must be unreachable from default
imports and product-proof examples.

### 1.1 Ordinary Public Contract

- ideal contract: safe for Tier 1 users through default imports or product
  examples. It has behavior, typed errors, tests, docs that do not call it a
  skeleton, and no requirement to learn engine internals.
- current examples that are close: `leaven_lm::LmRequest`, `Messages`,
  `LmResponse`, and `LmError` have concrete modules and contract tests
  (`crates/leaven-lm/src/lib.rs:1-27`,
  `crates/leaven-lm/tests/lm_contract.rs:9-150`).
- blocker/gap: many default-facing names do not meet this bar.
- correction direction: only names in this category may appear in
  `leaven::prelude::*`, default features, or product-proof examples.
- required proof/tests: default import compile tests, public contract tests, and
  example use through the ordinary product path.

### 1.2 Advanced Public Contract

- ideal contract: safe for GEPA customizers, optimizer authors, provider authors,
  or backend authors. It may expose `RunContext`, stage traits, `CachedLm`, store
  traits, or topology-specific knobs, but not through ordinary imports.
- current examples: `CachedLm<M, C>` is a real advanced wrapper with policy
  behavior and tests (`crates/leaven-lm-cache/src/cached.rs:6-116`,
  `crates/leaven-lm-cache/tests/cache_contract.rs:76-150`). `RunContext` is the
  designed optimizer-author mutation/finalization path
  (`docs/specs/initial_library.md:1827-1916`).
- blocker/gap: advanced names are currently mixed into ordinary prelude
  (`crates/leaven/src/prelude.rs:8-25`).
- correction direction: provide deliberate advanced namespaces/preludes and keep
  ordinary prelude narrow.
- required proof/tests: export ledger proves advanced names are not in ordinary
  prelude but remain reachable through explicit advanced paths.

### 1.3 Test-Support Public

- ideal contract: public only because examples/tests need it. The path/name says
  fake, mock, fixture, test, or demo.
- current acceptable direction: deterministic fake runtimes and scripted mock LMs
  can be valid when honestly named. The audit conventions permit scaffolding
  only when named and scoped as scaffolding
  (`reviews/2026-05-11-fuckery-extermination-today/auditing-conventions.md:49-59`).
- blocker/gap: fake/test support must not be in default product proof.
- correction direction: use explicit module/feature names such as `mock`,
  `fake`, `fixture`, or `test_support`.
- required proof/tests: public-maturity gate allowlists the name and prevents
  ordinary prelude/product-proof examples from depending on it as production
  behavior.

### 1.4 Explicit Scaffold Feature

- ideal contract: compile-time placeholder for planned crate placement only. The
  feature name says scaffold or experimental. It is never enabled by default and
  never used as proof.
- current blocker/gap: provider/backend features such as `lm-anthropic`,
  `workspace-docker`, `workspace-e2b`, and `store-sqlite` look like product
  integrations but expose only inert public structs
  (`crates/leaven/Cargo.toml:49-55`,
  `crates/leaven-lm-anthropic/src/client.rs:1`,
  `crates/leaven-workspace-docker/src/factory.rs:1`,
  `crates/leaven-store-sqlite/src/store.rs:1`).
- correction direction: remove these ordinary feature exports until real, or
  rename them as scaffold features with an allowlist and no product-example use.
- required proof/tests: feature-maturity test fails if a non-scaffold feature
  exposes only public unit structs or no capability trait impl.

### 1.5 Private Fixture

- ideal contract: useful local fixture that is not part of public API and is not
  imported by product-proof examples.
- current blocker/gap: `ReflectiveMutation` is a deterministic fixed-edit
  fixture (`crates/leaven-gepa/src/proposer.rs:21-47`) but is public and
  production-looking (`crates/leaven-gepa/src/lib.rs:18-34`).
- correction direction: rename to `FixedEditProposer` or equivalent and move it
  to tests/examples or an explicit fixture module.
- required proof/tests: product examples cannot import private fixtures; GEPA
  reflection tests use the real reflector contract.

### 1.6 Stale Skeleton Metadata

- ideal contract: package descriptions and crate docs tell current truth. A
  behavior-bearing crate should not still call itself a skeleton, and a skeleton
  label alone is not enough to classify every export in the crate as inert.
- current blocker/gap: `rg` finds skeleton metadata on both behavior-bearing
  crates and true placeholder crates. `leaven-lm-openai` still has skeleton
  package metadata (`crates/leaven-lm-openai/Cargo.toml:3`) while its client has
  real OpenAI request lowering and response parsing
  (`crates/leaven-lm-openai/src/client.rs:39-160`). By contrast,
  `leaven-lm-anthropic` exposes only inert client/config names
  (`crates/leaven-lm-anthropic/src/client.rs:1`,
  `crates/leaven-lm-anthropic/src/config.rs:1`).
- correction direction: stale metadata is its own finding class. Clean it when
  behavior exists; scaffold-gate or remove exports when behavior does not.
- required proof/tests: the public-maturity gate must report skeleton metadata
  separately from public inert symbols, and must not allow crate-wide
  placeholder exemptions.

## 2. Facade And Default Feature Requirements

### 2.1 `leaven` Umbrella

- ideal contract: `leaven` is import experience only; it re-exports ordinary
  product entrypoints and explicit advanced namespaces, not implementation logic
  (`crates/leaven/src/lib.rs:1-4`).
- current implementation: `leaven` re-exports cold algebra, engine internals, LM
  vocabulary, run builder, surface vocabulary, derive macros, std facade, GEPA,
  workspace, agentic, LM cache, and providers
  (`crates/leaven/src/lib.rs:16-75`).
- blocker/gap: no distinction between ordinary user imports and engine-author or
  wrapper/provider imports.
- correction direction:
  - `leaven::prelude` contains only ordinary product contracts;
  - `leaven::advanced` or crate modules expose engine-author traits explicitly;
  - `leaven::lm`, `leaven::run`, `leaven::surface`, etc. may remain clear module
    aliases if they do not imply default product maturity;
  - no compile-error derive macros in defaults.
- required proof/tests:
  - a default import compile test that uses only `optimize`, ordinary score/run
    types, budget, and behavior-bearing LM/request types;
  - a negative or ledger test proving `RunContext`, `RunGraphView`, raw contexts,
    derive macros, placeholder provider/backend types, and fixtures are absent
    from ordinary prelude.
  - an export-route ledger generated from `crates/leaven/src/lib.rs`,
    `crates/leaven/src/prelude.rs`, `crates/leaven-std/src/lib.rs`, and
    feature-gated provider/backend re-exports.

### 2.2 Default Features

- ideal contract: default features must be behavior-bearing and ordinary-useful.
- current implementation: defaults are `std`, `derive`, and `gepa`
  (`crates/leaven/Cargo.toml:38-42`).
- blocker/gap:
  - `derive` exposes compile-error macros
    (`crates/leaven-derive/src/unimplemented.rs:3-8`);
  - `std` re-exports skeleton/placeholder vocabulary
    (`crates/leaven-std/src/lib.rs:3-60`,
    `crates/leaven-artifacts/src/lib.rs:1-28`,
    `crates/leaven-render/src/lib.rs:1-23`);
  - `gepa` exposes fixed reflection fixture and placeholders
    (`crates/leaven-gepa/src/proposer.rs:21-56`,
    `crates/leaven-gepa/src/optimizer.rs:716-722`).
- correction direction: remove each default until it passes public maturity, or
  land the missing behavior and tests first.
- required proof/tests: feature-maturity gate checks every default feature for
  behavior-bearing public contract tests and rejects any default dependency on an
  explicit scaffold feature.

### 2.3 `leaven-std`

- ideal contract: shallow curated facade over standard pieces, not an
  implementation bucket (`AGENTS.md:21-27`).
- current implementation: it re-exports artifacts, evidence, preferences,
  populations, renderers, surfaces, and their preludes wholesale
  (`crates/leaven-std/src/lib.rs:3-60`).
- blocker/gap: wholesale re-export makes inert names look standard.
- correction direction: curate behavior-bearing names only. Keep future standard
  names out of `leaven-std` until they carry behavior and tests.
- required proof/tests: `leaven-std` export ledger and public-stub denylist.

### 2.4 Feature-Gated Provider And Backend Exports

- ideal contract: a non-scaffold feature exposes a usable adapter against its
  owning trait, with constructor/factory, typed errors, and at least one
  non-network law or mapping test.
- current implementation: `leaven` exposes provider/backend feature names
  (`crates/leaven/Cargo.toml:49-55`) and provider re-exports
  (`crates/leaven/src/lib.rs:68-75`), while several enabled crates expose only
  public unit structs (`crates/leaven-lm-anthropic/src/client.rs:1`,
  `crates/leaven-lm-local/src/client.rs:1`,
  `crates/leaven-workspace-docker/src/factory.rs:1`,
  `crates/leaven-workspace-e2b/src/factory.rs:1`,
  `crates/leaven-store-object/src/store.rs:1`,
  `crates/leaven-store-sqlite/src/store.rs:1`).
- blocker/gap: a feature name is a public capability promise. Optional does not
  mean scaffold.
- correction direction: remove the feature/export until real, or rename into an
  explicit scaffold feature that cannot be enabled by default and cannot be used
  as product proof.
- required proof/tests: every non-scaffold provider/backend feature has a trait
  implementation compile test and a non-network behavior law.

## 3. Topology And Crate Graph Requirements

### 3.1 Existing Topology Must Stay

- ideal contract: dependency direction protects knowledge boundaries. Foundation,
  engine, product builder, LM/cache, provider, optimizer, and umbrella edges are
  specified in topology docs
  (`docs/specs/leaven_v0_2_1b_corrected_crate_topology_lib_rs.md:257-631`).
- current implementation: dependency tests encode the current graph
  (`crates/leaven/tests/topology_contract.rs:250-418`) and verify it
  (`crates/leaven/tests/topology_contract.rs:445-459`).
- blocker/gap: dependency topology is necessary and should not be weakened.
- correction direction: keep exact dependency checks while adding maturity checks.
- required proof/tests: existing dependency-edge checks continue to pass after
  maturity gate additions.

### 3.2 Orphan Directory Rejection

- ideal contract: every `crates/*` directory is a workspace crate or absent.
- current implementation: `crates/leaven-dsrs` exists with source files but no
  `Cargo.toml` or `src/lib.rs`; topology spec still names it
  (`docs/specs/leaven_v0_2_1b_corrected_crate_topology_lib_rs.md:120-136`).
- blocker/gap: stale directories create false routing targets.
- correction direction: delete orphan or hard-cut it into a real crate.
- required proof/tests: topology test scans `crates/*` and rejects directories
  not in workspace membership unless explicitly allowlisted with a deletion date.

### 3.3 Crate Root Public Export Ledger

- ideal contract: `lib.rs` files are maps: module declarations, curated
  re-exports, optional preludes. Public `pub mod` paths must be deliberate stable
  namespaces.
- current implementation: GEPA publicly exposes file-layout modules like `gate`,
  `optimizer`, `part_selector`, `proposer`, `selector`, and `validation`
  (`crates/leaven-gepa/src/lib.rs:3-8`). Similar patterns are already recorded in
  cross-cutting topology findings for workspace and agent crates.
- blocker/gap: downstream code can depend on file layout instead of durable
  concepts.
- correction direction: make modules private unless the module path itself is a
  stable public namespace.
- required proof/tests: crate-root export ledger with allowlist for public module
  paths.

### 3.4 Public Stub Denylist

- ideal contract: public unit structs in production modules must be meaningful
  marker types with documented laws, or they are scaffolding.
- current implementation: many public unit structs are one-line placeholders,
  e.g. `BeamPopulation`, `MapElites`, `AnthropicLm`, `DockerWorkspaceFactory`,
  `ObjectStore`, and `SqliteStore`
  (`crates/leaven-population/src/beam.rs:1`,
  `crates/leaven-population/src/map_elites.rs:1-3`,
  `crates/leaven-lm-anthropic/src/client.rs:1`,
  `crates/leaven-workspace-docker/src/factory.rs:1`,
  `crates/leaven-store-object/src/store.rs:1`,
  `crates/leaven-store-sqlite/src/store.rs:1`).
- blocker/gap: tests do not distinguish valid zero-sized marker types from
  placeholder public capability names.
- correction direction: add allowlist with required rationale: marker law,
  test-support, explicit scaffold, or delete/implement.
- required proof/tests: generated public-unit-struct ledger and CI check.

The public-unit-struct ledger must use exact symbol rows. Whole-crate allowlists
are not acceptable because mixed crates are common. For example, a
behavior-bearing population crate can keep `KeepBest` and `ParetoFrontier` while
still failing `BeamPopulation` or `MapElites` until they carry laws and tests.

## 4. LM And Cache Composition Requirements

### 4.1 Provider-Neutral LM Contract

- ideal contract: `LmRequest` carries model, messages, sampling, output,
  continuation, and provider hints; canonical messages are multi-turn truth, and
  continuation is transport state
  (`docs/specs/lm_runtime_and_response_cache.md:91-152`).
- current implementation: `LmRequest` and `LmContinuation` match this shape
  (`crates/leaven-lm/src/request.rs:7-86`), with message/request tests
  (`crates/leaven-lm/tests/lm_contract.rs:9-110`).
- blocker/gap: current proof is local to LM crate; product examples bypass it.
- correction direction: product examples and run/runtime roles must consume the
  same LM contract.
- required proof/tests: product-level LM call path through solver and reflector
  roles.

### 4.2 Response Cache Contract

- ideal contract: cache keys include provider fingerprint, model, messages,
  sampling, output, and provider hints, and exclude API keys, response IDs,
  `previous_response_id`, wall-clock time, and backend paths
  (`docs/specs/lm_runtime_and_response_cache.md:154-206`).
- current implementation: `CachedLm` applies policies and returns zero-cost cache
  hits (`crates/leaven-lm-cache/src/cached.rs:53-109`), with policy tests
  (`crates/leaven-lm-cache/tests/cache_contract.rs:76-150`).
- blocker/gap: cache policy is not wired through product roles, and ordinary docs
  currently show wrapper stacking.
- correction direction: expose role policy in `leaven-run` or a runtime
  composition root. Keep `CachedLm` reachable for advanced users.
- required proof/tests: role-level cache test plus low-level cache-key law tests.
  The role-level test must prove at least two independently configured roles
  such as solver and reflector; one role hitting cache must not imply another
  role hits or shares policy.

### 4.3 Provider Exposure

- ideal contract: concrete provider features expose usable providers or stay out
  of the umbrella.
- current implementation: `OpenAiLm` is partially real but accepts and ignores a
  default model argument (`crates/leaven-lm-openai/src/client.rs:27-37`) while its
  fingerprint excludes model state (`crates/leaven-lm-openai/src/client.rs:124-134`).
  Anthropic/local are inert one-line structs.
- blocker/gap: public provider APIs give inconsistent mental models.
- correction direction:
  - for OpenAI, either store and use a real default model or remove the argument;
  - for inert providers, remove feature/export or implement real adapters.
- required proof/tests: provider mapping tests and constructor/fingerprint law.

### 4.4 GEPA Cache Boundary

- ideal contract: GEPA consumes `impl Lm`; cache policy is configured above GEPA.
- current implementation: LM spec says `leaven-gepa -> leaven-lm, not
  leaven-lm-cache` (`docs/specs/lm_runtime_and_response_cache.md:44-52`), but
  GEPA spec both allows and forbids `leaven-lm-cache`
  (`docs/specs/gepa_optimizer_surface.md:174-197`).
- blocker/gap: implementors cannot tell where cache belongs.
- correction direction: choose the LM spec boundary: GEPA does not depend on
  cache stores; run/runtime role composition owns cache policy.
- required proof/tests: topology contract and docs agree on no
  `leaven-gepa -> leaven-lm-cache` edge.

## 5. Example And Proof Requirements

### 5.1 Example Classification

- ideal contract: examples are classified as `product-proof`,
  `mechanics-smoke`, or `proxy-demo`.
- current implementation: coverage runs all milestone packages, including
  `p8_aime_gepa` (`scripts/coverage-gate.py:13-24`), while the AIME README says
  deterministic AIME is not evidence of live improvement
  (`examples/p8_aime_gepa/README.md:9-33`).
- blocker/gap: coverage does not tell whether a product path was exercised.
- correction direction: add classification and make product maturity depend only
  on product-proof examples.
- required proof/tests: coverage output/check includes classification; product
  proof cannot import fixtures or shell out to provider bypasses.

### 5.2 AIME/GEPA Product Proof

- ideal contract: AIME/GEPA proof uses `leaven::optimize`, train/validation/test
  semantics, runner/scorer or evaluator, real GEPA reflection, LM/cache roles,
  evidence, graph, budget, events, and result facade.
- current implementation: AIME uses `ReflectiveMutation::new` with a hard-coded
  optimized prompt (`examples/p8_aime_gepa/src/main.rs:80-99`) and shells out to
  Python for live OpenAI solving (`examples/p8_aime_gepa/src/main.rs:293-301`).
- blocker/gap: it proves public builder mechanics and deterministic score
  movement, not live Leaven LM/cache/reflection.
- correction direction: keep deterministic mechanics smoke if useful, but it
  cannot be the product-proof gate. The product-proof variant must route solver
  and reflector through Leaven APIs.
- required proof/tests: AIME product proof with mock LM by default and optional
  OpenAI provider swap requiring minimal config, not code changes.

### 5.3 Product-Proof Example Minimum Bar

A `product-proof` example must satisfy all of these:

1. It starts from the ordinary public entry surface, not a crate-internal helper.
2. It runs train/search cases and reports validation/test according to
   `leaven-run` split semantics.
3. It routes solver/program execution and reflector/proposer execution through
   Leaven-owned traits or role configuration.
4. It records evidence, graph mutation, budget/cost, events, and result facade
   truth through Leaven surfaces.
5. It does not shell out to provider bypasses or import production-looking
   fixtures.
6. Optional live-provider mode is a config/env swap over the same Leaven path,
   not a different implementation path.

`mechanics-smoke` examples may use deterministic fixtures. `proxy-demo` examples
may exercise external scripts or intentionally bypass a missing Leaven surface.
Neither category can satisfy a public-maturity or release-readiness claim.

## 6. Minimum Cross-Cutting Exit Criteria

A future implementation slice cannot claim cross-cutting maturity until all of
these are true:

1. `cargo test -p leaven --test topology_contract` enforces dependency topology
   plus public maturity.
2. `leaven::prelude::*` default imports expose no compile-error derives, inert
   provider/backend names, production-looking fixtures, or engine-author-only
   raw contexts.
3. `leaven-std` re-exports only behavior-bearing standard pieces or is not a
   default-facing facade.
4. GEPA is default-facing only when real reflection and required slots are
   implemented and tested.
5. LM/cache composition is configured by run/runtime role for ordinary users,
   while `CachedLm` remains advanced.
6. Provider/backend features expose real adapters or are removed from ordinary
   feature names.
7. `crates/*` has no orphan non-workspace directories.
8. Product-proof examples are separated from mechanics/proxy demos and at least
   one product-proof example exercises Leaven-owned run, GEPA, LM/cache, evidence,
   graph, budget, events, and result surfaces.
9. Stale skeleton metadata is either cleaned for behavior-bearing crates or
   moved behind explicit scaffold status for true placeholders.
10. `just check` passes after the slice, because docs/specs and topology tests are
   part of the public contract.
