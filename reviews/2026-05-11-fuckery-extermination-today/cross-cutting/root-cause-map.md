# Cross-Cutting Root Cause Map

Status: canonical cross-cutting audit doc.

Scope: crate graph, public maturity, facades/features, LM/cache composition,
topology tests, examples, and scaffolding classification. This is not a
Layer 1/2/3 root-cause map; it records the causes that cut across those layers
and make the public repo shape overstate what Leaven can currently prove.

## RC-X-001: Boundary Existence Is Being Treated As Capability Evidence

- severity: blocker
- surface: crate graph, topology tests, public facades
- ideal contract: the crate graph is a knowledge-boundary map. The original
  design says Leaven is a Rust optimizer library over arbitrary artifacts, not a
  GEPA-only engine, and cold core must not assume scalar scores, GEPA loop
  shape, one-shot LM calls, or train/validation presence
  (`docs/specs/initial_library.md:406-443`). The corrected topology spec says
  dependency direction should be CI-enforced
  (`docs/specs/leaven_v0_2_1b_corrected_crate_topology_lib_rs.md:257-314`),
  but it does not say crate shells prove product maturity.
- current implementation: the topology contract checks expected workspace
  members and that each expected crate has a manifest plus `src/lib.rs`
  skeleton (`crates/leaven/tests/topology_contract.rs:420-443`), then checks
  dependency edges (`crates/leaven/tests/topology_contract.rs:445-459`) and a
  small set of leak rules (`crates/leaven/tests/topology_contract.rs:461-505`).
  It does not reject public stubs, skeleton descriptions, orphan `crates/*`
  directories, or default facade exports of non-working public names.
- blocker/gap: a future implementor can satisfy topology by adding a manifest
  and `src/lib.rs` while leaving the capability inert. That is precisely the
  false proof pattern the review tree exists to kill.
- user impact: ordinary users, GEPA customizers, and optimizer authors see a
  large crate graph and assume capability exists. They route work into public
  names that compile but do not carry behavior or laws.
- correction direction: keep dependency topology, but add a public-maturity
  layer to topology: every default-facing crate/export must be classified as
  ordinary public contract, advanced public contract, test-support public,
  explicit scaffold, or private fixture.
- required proof/tests: extend `crates/leaven/tests/topology_contract.rs` to
  fail on unregistered `crates/*` directories, skeleton package/module docs in
  non-scaffold crates, public unit structs in ordinary features unless
  allowlisted, and umbrella/std/prelude exports of scaffold crates. The gate
  must enumerate the current export source, not only the crate name: root
  `pub use`, prelude `pub use`, crate-root `pub mod`, feature-gated re-export,
  and example import each need a category.

The maturity layer must not be a fuzzy label. Each public or proof-bearing name
needs a ledger row with:

```text
path + line
owning crate
export route: root | prelude | feature | example | test-support
category: ordinary | advanced | test-support | explicit-scaffold | private-fixture
behavior proof: test path, law name, example classification, or "none"
decision: keep public | move advanced | move fixture | remove/export-gate | implement
```

Unknown rows fail the gate. `none` is allowed only for explicit scaffold or
private fixture rows, never for default-facing rows.

## RC-X-002: Default Import Paths Promote Scaffolding Into Product API

- severity: blocker
- surface: `leaven`, `leaven-std`, derive, provider/backend features
- ideal contract: Tier 1 users should have a short path through
  `optimize(...).train(...).score(...).using(Gepa...)...run().await` and should
  not learn internal traits (`docs/specs/initial_library.md:451-468`,
  `docs/specs/initial_library.md:3608-3622`). The umbrella crate is only an
  import experience (`crates/leaven/src/lib.rs:1-4`), and root guidance says
  `leaven-std` is a shallow curated facade rather than an implementation bucket
  (`AGENTS.md:21-27`).
- current implementation: `leaven` defaults enable `std`, `derive`, and `gepa`
  (`crates/leaven/Cargo.toml:38-42`). The umbrella root re-exports engine-author
  concepts like `RunContext`, `RunGraphView`, `TrustPolicy`, stage traits, and
  raw contexts (`crates/leaven/src/lib.rs:16-42`). The common prelude exports
  those same internals plus derive macros, GEPA prelude, std prelude, and
  LM-cache prelude (`crates/leaven/src/prelude.rs:3-49`).
- blocker/gap: default features and preludes are the highest-signal public
  maturity test. Today they expose compile-error derive macros
  (`crates/leaven-derive/src/lib.rs:9-33`,
  `crates/leaven-derive/src/unimplemented.rs:3-8`), inert standard names
  (`crates/leaven-artifacts/src/lib.rs:1-28`,
  `crates/leaven-render/src/materializer.rs:1-5`), and GEPA fixture names
  (`crates/leaven-gepa/src/proposer.rs:21-56`).
- user impact: a user can start from `leaven::prelude::*` and immediately import
  names that either do not work or teach the wrong layer. This makes the library
  feel much more complete than it is.
- correction direction: hard-cut default imports to behavior-bearing ordinary
  contracts only. Move engine-author imports to advanced namespaces, remove
  derive from defaults until implemented, and keep std/provider/backend
  placeholders out of ordinary facades.
- required proof/tests: add compile tests for default `leaven` imports proving
  no compile-error macros are re-exported by default; add export-ledger tests for
  `leaven::prelude`, `leaven-std::prelude`, and feature-gated provider/backend
  modules.

## RC-X-003: LM Cache Is Real, But Its Layer Placement Is Publicly Unsettled

- severity: high
- surface: LM runtime, response cache, GEPA/run composition
- ideal contract: `leaven-lm` owns provider-neutral request/response vocabulary;
  `leaven-lm-cache` owns response-cache policy, keys, store trait, in-memory
  backend, and `CachedLm`; concrete providers stay outside optimizers; GEPA
  consumes `impl Lm` and does not know cache stores
  (`docs/specs/lm_runtime_and_response_cache.md:33-57`). Multi-turn requests use
  canonical `messages`, while provider continuations are transport state and not
  cache identity (`docs/specs/lm_runtime_and_response_cache.md:91-119`).
- current implementation: `leaven-lm` has real provider-neutral modules and
  prelude (`crates/leaven-lm/src/lib.rs:1-27`), and tests for messages, response
  role validation, token cost, request defaults, and errors
  (`crates/leaven-lm/tests/lm_contract.rs:9-150`). `leaven-lm-cache` implements
  a real `CachedLm<M, C>` wrapper and cache policies
  (`crates/leaven-lm-cache/src/cached.rs:6-116`) with policy tests
  (`crates/leaven-lm-cache/tests/cache_contract.rs:76-150`). But the LM spec's
  product example teaches manual wrapper stacking
  (`docs/specs/lm_runtime_and_response_cache.md:17-28`), and the user explicitly
  flagged `CachedLm` as a smell in the public API
  (`reviews/2026-05-11-fuckery-extermination-today/complaints/session-user-messages-for-codex.md:292-299`).
- blocker/gap: the cache crate is not fake; the problem is public composition.
  Ordinary users need runtime/cache policy by role, not wrapper topology. GEPA
  also has a spec contradiction: one section allows `leaven-gepa` to depend on
  `leaven-lm-cache`, then immediately forbids it
  (`docs/specs/gepa_optimizer_surface.md:174-197`), while the live manifest and
  topology contract omit the cache edge
  (`crates/leaven/tests/topology_contract.rs:272-283`).
- user impact: the same cache concept can be implemented three ways by future
  authors: manual wrapper in user code, GEPA-local cache policy, or run/runtime
  role configuration. That splits the product path and risks duplicating cache
  behavior.
- correction direction: keep `CachedLm` as an advanced implementation and
  testing/power-user wrapper. Make ordinary product paths configure solver,
  reflector, scorer/judge, and agent runtime cache policy by role in `leaven-run`
  or a small runtime composition root above GEPA. The public shape should name
  roles, not wrapper topology: `solver`, `reflector`, `scorer` or `judge`, and
  `agent-runtime` are the configuration points that need independent cache and
  cost policy.
- required proof/tests: add a product-level LM runtime/cache scenario that runs
  one solver call and one reflector call through `leaven-lm`, `leaven-lm-cache`,
  and `leaven-lm-mock` or a non-network provider test; prove cache hit cost,
  continuation exclusion from cache identity, and independent role policy.

## RC-X-004: Provider And Backend Names Are Exposed Before Capability Laws Exist

- severity: high
- surface: optional features, backend crates, provider crates
- ideal contract: feature names should communicate available capability. The LM
  spec says OpenAI/Anthropic/local runtimes and cache backends must stay out of
  optimizer crates (`docs/specs/lm_runtime_and_response_cache.md:30-42`), but a
  provider crate exposed by feature should still implement its capability law.
- current implementation: `leaven` exposes provider/backend features such as
  `lm-openai`, `lm-anthropic`, `lm-cache`, `workspace-docker`, `workspace-e2b`,
  and `store-sqlite` (`crates/leaven/Cargo.toml:49-55`) and re-exports provider
  crates when features are enabled (`crates/leaven/src/lib.rs:68-75`). Some
  exposed crates are one-line inert public structs:
  `AnthropicLm`, `LocalLm`, `DockerWorkspaceFactory`, `E2bWorkspaceFactory`,
  `ObjectStore`, and `SqliteStore`
  (`crates/leaven-lm-anthropic/src/client.rs:1`,
  `crates/leaven-lm-local/src/client.rs:1`,
  `crates/leaven-workspace-docker/src/factory.rs:1`,
  `crates/leaven-workspace-e2b/src/factory.rs:1`,
  `crates/leaven-store-object/src/store.rs:1`,
  `crates/leaven-store-sqlite/src/store.rs:1`).
- blocker/gap: optional is not the same as scaffold. A production-looking feature
  gate that exposes only an inert type is still a public lie.
- user impact: a user can enable a provider/backend feature and get no
  constructor, no trait impl, no typed error, no contract test, and no working
  integration path.
- correction direction: either implement adapter capability against the owning
  trait with tests, or remove the umbrella feature/export. Reserved future names
  belong only behind explicit scaffold features that cannot be confused with
  production integration.
- required proof/tests: provider/backend feature tests must instantiate the
  adapter, prove the relevant trait impl, and run at least one non-network law or
  mapping test. Umbrella features must fail the maturity gate if they expose only
  public unit structs.

## RC-X-005: GEPA Is Default-Facing Before Its Cross-Cutting Dependencies Are Honest

- severity: blocker
- surface: GEPA public imports, LM reflection, evidence, examples
- ideal contract: GEPA is one optimizer value composed from parent selector, part
  selector, batch sampler, reflector/proposer, acceptance policy, validation
  policy, population/frontier, and merge scheduler
  (`docs/specs/initial_library.md:443`,
  `docs/specs/gepa_optimizer_surface.md:232-293`). Its step must capture
  assessment IDs, propose edits, lower surface edits, apply through `RunContext`,
  evaluate children, and preserve causal/informed-by provenance
  (`docs/specs/gepa_optimizer_surface.md:320-357`).
- current implementation: `gepa` is in the umbrella default feature set
  (`crates/leaven/Cargo.toml:38-42`) and the common prelude re-exports
  `leaven_gepa::prelude::*` (`crates/leaven/src/prelude.rs:33-34`). The GEPA
  crate root publicly exposes file-layout modules and placeholder names
  (`crates/leaven-gepa/src/lib.rs:3-35`). `SurfaceProposer` only sees artifact,
  surface, and part (`crates/leaven-gepa/src/proposer.rs:6-19`), while
  `ReflectiveMutation` is documented as a deterministic fixture and always
  returns one stored edit (`crates/leaven-gepa/src/proposer.rs:21-47`).
- blocker/gap: GEPA reflection currently cannot name feedback assessments,
  casewise evidence, trace excerpts, objective/background, LM runtime/cache role,
  or budget handles, even though the spec requires those inputs
  (`docs/specs/gepa_optimizer_surface.md:447-483`).
- user impact: examples can show score movement without proving the main GEPA
  capability. `p8_aime_gepa` wires the reflector to a hard-coded prompt
  replacement (`examples/p8_aime_gepa/src/main.rs:80-99`) and its live solver
  shells out to Python instead of Leaven LM/cache
  (`examples/p8_aime_gepa/src/main.rs:271-301`).
- correction direction: remove GEPA from default-facing product imports until
  the real reflection contract exists, or land that contract. Rename fixed edit
  proposal to an explicit fixture and keep it out of product-proof examples.
- required proof/tests: GEPA must have law/example tests for builder slot
  completeness, mock-LM reflective mutation from casewise feedback, split policy
  hiding validation/test content, typed proposer parse errors, and product
  scenarios through `leaven-run` (`docs/specs/gepa_optimizer_surface.md:623-652`).

## RC-X-006: Public Examples And Coverage Gates Do Not Separate Product Proof From Proxy Demo

- severity: high
- surface: examples, scripts, acceptance proof
- ideal contract: examples that claim product proof must exercise the public
  surface under test. The implementation plan intentionally orders prototypes to
  surface design problems early, with pairwise before GEPA parity and GEPA parity
  reproducing Python GEPA on the validated substrate
  (`docs/specs/initial_library.md:4638-4683`).
- current implementation: coverage runs all milestone packages including
  `p8_aime_gepa` (`scripts/coverage-gate.py:13-24`). The AIME README states the
  deterministic path uses a scripted solver and is not evidence of live AIME
  improvement (`examples/p8_aime_gepa/README.md:9-33`). The live solver path
  shells out to `openai_solver.py` (`examples/p8_aime_gepa/src/main.rs:293-301`).
- blocker/gap: coverage can ratify examples that explicitly disclaim product
  proof. That makes test/coverage green a weak signal for the optimizer library
  maturity the user is asking about.
- user impact: future implementors can improve coverage by maintaining proxy
  paths instead of routing the same scenario through Leaven-owned LM/cache,
  reflection, graph, evidence, and result facades.
- correction direction: classify each example as `product-proof`,
  `mechanics-smoke`, or `proxy-demo`. Only product-proof examples may be used as
  acceptance evidence for public maturity claims.
- required proof/tests: update coverage/verification tooling to emit example
  proof classifications, and add at least one product-proof scenario that runs
  solver and reflector roles through Leaven APIs rather than shell/process
  provider bypasses.

## RC-X-007: Placeholder Vocabulary Is Too Coarse To Guide Implementation

- severity: high
- surface: public maturity, crate inventory, implementation sequencing
- ideal contract: scaffolding is acceptable only when named and scoped as
  scaffolding; behavior-bearing code should not be condemned by stale metadata
  alone (`reviews/2026-05-11-fuckery-extermination-today/auditing-conventions.md:49-59`,
  `AGENTS.md:88-99`).
- current implementation: the live tree contains true placeholders, mixed crates,
  stale skeleton metadata, and real advanced surfaces in the same visual shape.
  `leaven-lm-cache` has real policy behavior and tests
  (`crates/leaven-lm-cache/src/cached.rs:6-116`,
  `crates/leaven-lm-cache/tests/cache_contract.rs:76-150`), while
  `AnthropicLm`, `DockerWorkspaceFactory`, and `SqliteStore` are one-line public
  structs (`crates/leaven-lm-anthropic/src/client.rs:1`,
  `crates/leaven-workspace-docker/src/factory.rs:1`,
  `crates/leaven-store-sqlite/src/store.rs:1`). Some behavior-bearing crates
  still carry stale skeleton descriptions, such as `leaven-lm-openai`
  (`crates/leaven-lm-openai/Cargo.toml:3`,
  `crates/leaven-lm-openai/src/client.rs:39-160`).
- blocker/gap: a broad "placeholder crate" label is not implementation-actionable.
  It can either excuse a production-looking inert export because "the crate is a
  scaffold" or wrongly demote real behavior because metadata still says
  skeleton.
- user impact: future implementors can fix the wrong thing: deleting useful
  advanced capability, polishing stale docs while leaving inert exports public,
  or allowlisting an entire crate instead of the exact public name that is safe.
- correction direction: split audit and gate vocabulary into:
  `true placeholder export`, `mixed crate with public stubs`,
  `stale skeleton metadata`, `advanced real surface in ordinary path`,
  `test-support public`, and `orphan/stale directory`.
- required proof/tests: public-maturity checks should allowlist or fail exact
  symbols/routes, not whole crates. A skeleton-description hit must require one
  of two actions: clean stale metadata when behavior exists, or mark the crate as
  explicit scaffold and remove it from default/product-proof routes.
