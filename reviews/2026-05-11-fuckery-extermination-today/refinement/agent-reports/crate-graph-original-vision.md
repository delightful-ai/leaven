# Crate Graph Against Original Vision

Date: 2026-05-11

Auditor: Codex, first-party refinement pass

## Short Answer

No. The current crate graph is aligned with the original Leaven vision as a
knowledge-boundary map, but not yet as an implemented library surface. The graph
names many of the right boundaries, and several foundation crates are
behavior-bearing. The failure is that default features, facades, preludes,
topology docs, and examples make some scaffolded or partial boundaries look like
finished optimizer-library capability.

The first-pass crate graph audit is directionally right. The main refinement is
that "placeholder" is not the same thing as "bad": private or explicitly named
scaffolding is acceptable. A public production-looking name, default feature, or
facade export that points to inert code is a public lie. Conversely, a stale
`crate skeleton` description on a behavior-bearing crate is not enough to call
that crate a placeholder; the audit must inspect live modules and tests before
freezing that label.

## Findings

### CGV-001: The crate graph encodes boundaries, not product truth

- `id`: CGV-001
- `severity`: high
- `vision promise`: The original vision is compiler-enforced knowledge
  boundaries plus working optimizer surfaces. `initial_library` says the engine
  owns infrastructure, the optimizer owns algorithm rhythm, stages own
  side-effectful work, and context methods centralize graph, budget, cache,
  trust, and callback correctness (`docs/specs/initial_library.md:581-653`).
  The root repo contract maps those responsibilities by crate
  (`AGENTS.md:10-27`) and requires topology discipline before placement
  decisions (`AGENTS.md:33-42`).
- `current audit coverage`: The first-pass audit correctly says unfinished
  behavior is often exposed through names, features, examples, and topology docs
  (`reviews/2026-05-11-fuckery-extermination-today/cross-cutting/agent-report-crate-graph.md:13-24`).
  It also asks to extend topology checks beyond dependency drift
  (`reviews/2026-05-11-fuckery-extermination-today/cross-cutting/agent-report-crate-graph.md:251-288`).
- `gap`: The audit should explicitly say that crate existence is not evidence of
  vision alignment. The topology contract checks membership, manifests,
  `src/lib.rs` presence, exact dependency edges, a narrow cold-core leak class,
  and Codex protocol leaf-ness (`crates/leaven/tests/topology_contract.rs:421-505`).
  It does not prove that each public crate contract has behavior, laws, or
  acceptable public exports.
- `correction`: Treat the crate graph as a boundary ledger, not a capability
  ledger. Add topology/readiness tests that fail on unapproved public stubs,
  skeleton package descriptions, orphan `crates/*` directories, compile-error
  macros in default-facing imports, and facade/prelude re-exports of scaffold
  modules.
- `evidence`:
  - `docs/specs/initial_library.md:581-653`
  - `AGENTS.md:10-27`
  - `AGENTS.md:33-42`
  - `crates/leaven/tests/topology_contract.rs:421-505`
  - `reviews/2026-05-11-fuckery-extermination-today/cross-cutting/agent-report-crate-graph.md:251-288`

### CGV-002: Default features and facades make scaffolded behavior look ordinary

- `id`: CGV-002
- `severity`: blocker
- `vision promise`: Ordinary users should get a short, obvious product path and
  should not have to understand every internal trait (`docs/specs/initial_library.md:451-468`).
  The umbrella crate is an import experience only (`AGENTS.md:27`), and
  `leaven-std` is a shallow curated facade, not an implementation bucket
  (`AGENTS.md:21`).
- `current audit coverage`: The first-pass audit catches default derive exposure,
  `leaven-std` placeholder re-exports, and optional provider/backend feature
  stubs (`reviews/2026-05-11-fuckery-extermination-today/cross-cutting/agent-report-crate-graph.md:70-174`).
  The fix priority map also puts removal of placeholder exports in P4
  (`reviews/2026-05-11-fuckery-extermination-today/cross-cutting/fix-priority-map.md:56-64`).
- `gap`: This is stronger than "some stubs exist." The public import surface
  actively amplifies them: `leaven` defaults include `std`, `derive`, and `gepa`
  (`crates/leaven/Cargo.toml:38-42`); the umbrella prelude re-exports derive
  macros, GEPA prelude, std prelude, and LM cache prelude
  (`crates/leaven/src/prelude.rs:27-49`); `leaven-std` re-exports whole standard
  vocabulary crates (`crates/leaven-std/src/lib.rs:3-60`). That makes the graph
  feel more complete than it is.
- `correction`: Hard-cut default and prelude exports to behavior-bearing ordinary
  imports only. Reserved derive macros, future provider adapters, standard
  artifacts/renderers, and GEPA strategy placeholders should either be fully
  implemented with tests or absent from default-facing imports.
- `evidence`:
  - `docs/specs/initial_library.md:451-468`
  - `AGENTS.md:21-27`
  - `crates/leaven/Cargo.toml:38-55`
  - `crates/leaven/src/prelude.rs:27-49`
  - `crates/leaven-std/src/lib.rs:3-60`
  - `crates/leaven-derive/src/lib.rs:9-33`
  - `crates/leaven-derive/src/unimplemented.rs:3-8`

### CGV-003: GEPA is exposed as a default optimizer before its strategy boundary exists

- `id`: CGV-003
- `severity`: blocker
- `vision promise`: GEPA is one optimizer value composed from smaller GEPA
  strategies: parent selector, part selector, batch sampler, reflector/proposer,
  acceptance policy, validation policy, population/frontier, and optional merge
  proposer (`docs/specs/initial_library.md:443`). GEPA customizers should swap
  those parts without writing a new optimizer (`docs/specs/initial_library.md:470-485`).
  The GEPA optimizer spec requires a builder with those slots and rejection of
  incomplete/contradictory configurations (`docs/specs/gepa_optimizer_surface.md:271-304`).
- `current audit coverage`: The audit correctly identifies fixed
  `ReflectiveMutation`, placeholder strategy types, and a too-narrow
  `SurfaceProposer` as the worst false-positive path
  (`reviews/2026-05-11-fuckery-extermination-today/cross-cutting/agent-report-crate-graph.md:176-218`).
- `gap`: Because `gepa` is a default umbrella feature (`crates/leaven/Cargo.toml:38-42`)
  and the common prelude re-exports `leaven_gepa::prelude::*`
  (`crates/leaven/src/prelude.rs:33-34`), GEPA is presented as ordinary public
  capability. But the live GEPA root publicly exports file-layout modules and
  placeholder names (`crates/leaven-gepa/src/lib.rs:3-35`), while
  `ReflectiveMutation` is documented as a deterministic fixture that always
  returns the stored edit (`crates/leaven-gepa/src/proposer.rs:21-47`).
- `correction`: Keep a deterministic proposer only under an honest fixture or
  milestone-test name. Reserve `ReflectiveMutation` and default GEPA imports for
  the real async reflection path that consumes selected assessment/evidence/trace
  context and records `informed_by` provenance. Remove `gepa` from default-facing
  import paths until that contract exists.
- `evidence`:
  - `docs/specs/initial_library.md:443-485`
  - `docs/specs/gepa_optimizer_surface.md:271-304`
  - `docs/specs/gepa_optimizer_surface.md:322-355`
  - `crates/leaven/Cargo.toml:38-42`
  - `crates/leaven/src/prelude.rs:33-34`
  - `crates/leaven-gepa/src/lib.rs:3-35`
  - `crates/leaven-gepa/src/proposer.rs:21-56`

### CGV-004: Some scaffolding is acceptable, but only when it cannot be mistaken for the product

- `id`: CGV-004
- `severity`: medium
- `vision promise`: The GEPA implementation plan explicitly permits a
  deterministic proposer milestone before mock-LM reflection
  (`docs/specs/gepa_optimizer_surface.md:692-713`). The audit conventions allow
  scaffolding only when it is named and scoped as scaffolding, and mark it as a
  finding when it appears in ordinary public paths, umbrella exports, or examples
  as proof of real capability
  (`reviews/2026-05-11-fuckery-extermination-today/auditing-conventions.md:49-59`).
- `current audit coverage`: The first-pass audit often applies the right
  distinction: it calls `FakeAgentRuntime` low severity because it is honestly
  named and test/example oriented, while still questioning prelude placement
  (`reviews/2026-05-11-fuckery-extermination-today/cross-cutting/agent-report-crate-graph.md:415-444`).
- `gap`: The crate graph report should state the general rule up front. A private
  fixture, test-only helper, or explicitly named deterministic demo is acceptable
  scaffolding. A production-looking public type such as `ReflectiveMutation`,
  `GepaConfig`, `MergeScheduler`, `TextArtifact`, `ReflectionPromptRenderer`, or
  `SqliteStore` is not acceptable if it has no behavior. This distinction matters
  because the original user complaint is about substitution: examples and stubs
  claiming "implemented" while bypassing the real library surface
  (`reviews/2026-05-11-fuckery-extermination-today/complaints/session-user-messages-for-codex.md:600-611`).
- `correction`: Add a public-scaffolding classification to the audit and topology
  contract: `private fixture`, `test-support public`, `explicit scaffold
  feature`, `ordinary public contract`. Only the last category may appear in
  default facades and product examples.
- `evidence`:
  - `docs/specs/gepa_optimizer_surface.md:692-713`
  - `reviews/2026-05-11-fuckery-extermination-today/auditing-conventions.md:49-59`
  - `reviews/2026-05-11-fuckery-extermination-today/cross-cutting/agent-report-crate-graph.md:415-444`
  - `reviews/2026-05-11-fuckery-extermination-today/complaints/session-user-messages-for-codex.md:600-611`

### CGV-005: `leaven-dsrs` is not product-critical, but it is a topology lie while docs name it

- `id`: CGV-005
- `severity`: high
- `vision promise`: Domain/edge adapters may exist at the edge of the graph
  (`AGENTS.md:25`), but the topology spec presents `leaven-dsrs` as a workspace
  crate (`docs/specs/leaven_v0_2_1b_corrected_crate_topology_lib_rs.md:120-136`,
  `docs/specs/leaven_v0_2_1b_corrected_crate_topology_lib_rs.md:200-214`).
- `current audit coverage`: The first-pass audit correctly identifies
  `crates/leaven-dsrs` as an orphaned non-crate with only one-line public structs
  and no manifest/lib root
  (`reviews/2026-05-11-fuckery-extermination-today/cross-cutting/agent-report-crate-graph.md:39-68`).
- `gap`: The refinement is that the original optimizer-library vision does not
  depend on DSRS being present today. The lie is not "DSRS is missing"; the lie is
  stale topology truth. If `leaven-dsrs` is out of scope, the honest hard cut is
  deleting the orphan directory and removing spec/topology references. If it is
  in scope, it must become a real workspace crate.
- `correction`: Do not leave non-workspace `crates/*` directories as reminders.
  Add a topology test that every `crates/*` directory is either a workspace
  member or appears in a short explicit stale-directory allowlist with a deletion
  date. Prefer no allowlist here.
- `evidence`:
  - `AGENTS.md:25`
  - `docs/specs/leaven_v0_2_1b_corrected_crate_topology_lib_rs.md:120-136`
  - `docs/specs/leaven_v0_2_1b_corrected_crate_topology_lib_rs.md:200-214`
  - `crates/leaven/tests/topology_contract.rs:5-66`
  - `crates/leaven-dsrs/src/artifact.rs:1-3`
  - `crates/leaven-dsrs/src/bridge.rs:1`
  - `crates/leaven-dsrs/src/evaluator.rs:1`
  - `crates/leaven-dsrs/src/surface.rs:1`

### CGV-006: The first audit should not overstate placeholder status for behavior-bearing crates

- `id`: CGV-006
- `severity`: medium
- `vision promise`: Nearby names and metadata are not enough; tests should assert
  public/capability behavior, and code/doctrine mismatches should be resolved in
  the same change (`AGENTS.md:42`, `AGENTS.md:96-99`). Prior audit memory for this
  repo also warns that behavior-bearing crates can be misclassified if live
  wiring and tests are not checked.
- `current audit coverage`: The first-pass crate report already has a useful
  "mixed" category: it says `leaven-lm`, `leaven-lm-cache`, `leaven-lm-openai`,
  `leaven-agentic`, `leaven-population`, and others carry real behavior while
  still having public-surface problems
  (`reviews/2026-05-11-fuckery-extermination-today/cross-cutting/agent-report-crate-graph.md:466-511`).
- `gap`: Other ledgers and grep-based checks can still over-read stale
  `crate skeleton` descriptions. For example, `leaven-lm` has real
  provider-neutral vocabulary and contract tests (`crates/leaven-lm/src/lib.rs:1-27`,
  `crates/leaven-lm/tests/lm_contract.rs:9-120`); `leaven-lm-cache` has a real
  wrapper, policy behavior, and tests (`crates/leaven-lm-cache/src/cached.rs:6-116`,
  `crates/leaven-lm-cache/tests/cache_contract.rs:76-118`); `leaven-agentic` has
  real public wiring and integration tests (`crates/leaven-agentic/src/lib.rs:1-66`,
  `crates/leaven-agentic/tests/agentic_adapters.rs:34-143`); `KeepBest` and
  `ParetoFrontier` are behavior-bearing even though sibling population names are
  empty (`crates/leaven-population/src/keep_best.rs:1-98`,
  `crates/leaven-population/src/pareto_frontier.rs:1-160`,
  `crates/leaven-population/src/beam.rs:1`).
- `correction`: Split the audit vocabulary into `stale skeleton metadata`,
  `mixed crate with public stubs`, and `true placeholder crate`. Do not call a
  whole crate placeholder only because its manifest or module doc says
  "skeleton"; cite the behavior-bearing modules and the inert public exports
  separately.
- `evidence`:
  - `AGENTS.md:42`
  - `AGENTS.md:96-99`
  - `reviews/2026-05-11-fuckery-extermination-today/cross-cutting/agent-report-crate-graph.md:466-511`
  - `crates/leaven-lm/src/lib.rs:1-27`
  - `crates/leaven-lm/tests/lm_contract.rs:9-120`
  - `crates/leaven-lm-cache/src/cached.rs:6-116`
  - `crates/leaven-lm-cache/tests/cache_contract.rs:76-118`
  - `crates/leaven-agentic/src/lib.rs:1-66`
  - `crates/leaven-agentic/tests/agentic_adapters.rs:34-143`
  - `crates/leaven-population/src/keep_best.rs:1-98`
  - `crates/leaven-population/src/pareto_frontier.rs:1-160`
  - `crates/leaven-population/src/beam.rs:1`

### CGV-007: LM cache is a real crate, but the public layer is unsettled

- `id`: CGV-007
- `severity`: medium
- `vision promise`: Optimizer code should consume provider-neutral LM
  capabilities without knowing concrete providers or cache backends
  (`docs/specs/lm_runtime_and_response_cache.md:17-31`). The LM cache spec says
  `leaven-lm-cache` owns cache policy, keys, store trait, in-memory backend, and
  `CachedLm<M, C>` wrapper, while GEPA consumes `impl Lm` and not cache stores
  (`docs/specs/lm_runtime_and_response_cache.md:35-52`).
- `current audit coverage`: The LM/cache audit correctly says `CachedLm` is
  useful as an implementation and advanced-user shape but a bad ordinary Layer 1
  story if examples teach wrapper stacking
  (`reviews/2026-05-11-fuckery-extermination-today/cross-cutting/lm-and-cache-surface.md:17-48`).
  The crate graph audit also catches a GEPA spec contradiction: one section both
  allows and forbids `leaven-gepa -> leaven-lm-cache`
  (`reviews/2026-05-11-fuckery-extermination-today/cross-cutting/agent-report-crate-graph.md:220-249`).
- `gap`: This should not be framed as "cache crate fake." The wrapper and tests
  are real. The problem is layer placement: the same spec currently teaches
  ordinary user code to manually wrap an LM in `CachedLm`
  (`docs/specs/lm_runtime_and_response_cache.md:17-28`) while user feedback says
  cache composition touching the public API is a smell
  (`reviews/2026-05-11-fuckery-extermination-today/complaints/session-user-messages-for-codex.md:600-604`).
- `correction`: Keep `leaven-lm-cache` as an advanced/power-user capability
  crate, but update the Layer 1 product docs to configure cache policy through
  run/runtime roles. Resolve the GEPA `lm-cache` dependency contradiction in
  favor of GEPA consuming provider-neutral `Lm` and leaving response-cache
  composition above GEPA.
- `evidence`:
  - `docs/specs/lm_runtime_and_response_cache.md:17-52`
  - `docs/specs/lm_runtime_and_response_cache.md:156-204`
  - `docs/specs/gepa_optimizer_surface.md:174-198`
  - `crates/leaven-lm-cache/src/cached.rs:6-116`
  - `crates/leaven-lm-cache/tests/cache_contract.rs:76-118`
  - `reviews/2026-05-11-fuckery-extermination-today/cross-cutting/lm-and-cache-surface.md:17-48`
  - `reviews/2026-05-11-fuckery-extermination-today/complaints/session-user-messages-for-codex.md:600-604`

### CGV-008: Public `pub mod` paths turn implementation layout into semver surface

- `id`: CGV-008
- `severity`: medium
- `vision promise`: `lib.rs` files are maps only: module declarations, curated
  re-exports, and optional preludes; runtime/domain/helper logic and public test
  holes do not belong there (`AGENTS.md:35`). The corrected topology spec says
  the same for GEPA (`docs/specs/gepa_optimizer_surface.md:209-228`).
- `current audit coverage`: The first-pass audit catches public module-layout
  leakage in GEPA, workspace, agent-command, and Codex app-server crates
  (`reviews/2026-05-11-fuckery-extermination-today/cross-cutting/agent-report-crate-graph.md:326-354`).
- `gap`: This is not merely aesthetics. The original vision's "simple for
  models" rule depends on predictable factoring and conceptual names
  (`docs/specs/guiding_principles.md:60-68`, `docs/specs/guiding_principles.md:337-343`).
  Public file-layout modules let downstream code organize around unstable
  implementation buckets instead of durable concepts.
- `correction`: Add a topology/public-export test or ledger for crate roots:
  `pub mod` must be explicitly allowlisted as a stable namespace; otherwise
  modules should be private and only curated root/prelude exports should be
  public.
- `evidence`:
  - `AGENTS.md:35`
  - `docs/specs/guiding_principles.md:60-68`
  - `docs/specs/guiding_principles.md:337-343`
  - `docs/specs/gepa_optimizer_surface.md:209-228`
  - `crates/leaven-gepa/src/lib.rs:3-25`
  - `reviews/2026-05-11-fuckery-extermination-today/cross-cutting/agent-report-crate-graph.md:326-354`

### CGV-009: Topology docs need to prevent recurrence, not just describe the latest graph

- `id`: CGV-009
- `severity`: high
- `vision promise`: The corrected topology spec says correctness is prioritized
  over few crates, with the goal of compiler-enforced knowledge boundaries
  (`docs/specs/leaven_v0_2_1b_corrected_crate_topology_lib_rs.md:68`). The eval
  lowering spec already asks to extend topology tests for crate dependency
  constraints (`docs/specs/eval_lowering_detail.md:822-837`).
- `current audit coverage`: The first-pass report recommends generated ledgers
  for public unit structs, skeleton descriptions, orphan dirs, public exports,
  and example proof classification
  (`reviews/2026-05-11-fuckery-extermination-today/cross-cutting/agent-report-crate-graph.md:587-599`).
- `gap`: The recurrence mechanism is broader than the current report says:
  future agents can satisfy topology by creating a manifest and `src/lib.rs`
  skeleton because the topology test currently rewards exactly that
  (`crates/leaven/tests/topology_contract.rs:421-443`). That trains the wrong
  behavior unless paired with a public-stub denylist/allowlist and a capability
  contract ledger.
- `correction`: Add a "public maturity" layer to topology docs and tests:
  dependency allowlist, crate membership, orphan directory rejection, crate-root
  public export ledger, scaffold allowlist, example proof classification, and a
  rule that any crate appearing in default features must have at least one
  behavior-bearing public contract test or be explicitly excluded from defaults.
- `evidence`:
  - `docs/specs/leaven_v0_2_1b_corrected_crate_topology_lib_rs.md:68`
  - `docs/specs/eval_lowering_detail.md:822-837`
  - `crates/leaven/tests/topology_contract.rs:421-443`
  - `reviews/2026-05-11-fuckery-extermination-today/cross-cutting/agent-report-crate-graph.md:587-599`

## Refinement Edits Recommended

Update these integrated docs after folding in agent reports:

1. `reviews/2026-05-11-fuckery-extermination-today/refinement/vision-comparison.md`
   - Add the core distinction from this report: boundary-map alignment is not
     product alignment; default-facing public imports are the test for whether a
     scaffold has become a lie.
2. `reviews/2026-05-11-fuckery-extermination-today/refinement/surface-requirements.md`
   - Add the four public-scaffolding categories: private fixture, test-support
     public, explicit scaffold feature, ordinary public contract.
   - Clarify that `CachedLm` may stay an advanced cache capability while Layer 1
     examples configure cache policy through runtime/run roles.
3. `reviews/2026-05-11-fuckery-extermination-today/refinement/implementation-sequence.md`
   - In Phase 0 and Phase 4, distinguish deleting false public paths from fixing
     stale skeleton metadata on behavior-bearing crates.
   - Add an exit criterion that default `leaven` imports expose no compile-error
     derives, no inert standard names, and no placeholder provider/backend types.
4. `reviews/2026-05-11-fuckery-extermination-today/refinement/open-design-questions.md`
   - Add a question for the Layer 1 cache/runtime composition root and explicitly
     close the GEPA `leaven-lm-cache` dependency contradiction.
5. `reviews/2026-05-11-fuckery-extermination-today/cross-cutting/agent-report-crate-graph.md`
   - Refine the inventory categories to `real`, `mixed with public stubs`,
     `stale skeleton metadata`, `true placeholder`, and `orphan/stale directory`.
6. `reviews/2026-05-11-fuckery-extermination-today/cross-cutting/topology-and-crate-graph.md`
   - Add the public-maturity topology test requirements: orphan dir rejection,
     scaffold allowlist, public unit-struct deny/allow ledger, crate-root export
     ledger, and default-feature maturity check.
7. `docs/specs/gepa_optimizer_surface.md`
   - Resolve the contradictory `leaven-gepa` dependency treatment for
     `leaven-lm-cache`; favor GEPA consuming `leaven-lm` while cache composition
     lives above GEPA.
8. `docs/specs/lm_runtime_and_response_cache.md`
   - Separate advanced wrapper examples from ordinary Layer 1 runtime/cache
     policy examples so `CachedLm` does not become the default product story.
9. `docs/specs/leaven_v0_2_1b_corrected_crate_topology_lib_rs.md`
   - Remove or reintroduce `leaven-dsrs` consistently, and add public-maturity
     checks to the topology contract section instead of only membership and
     dependency edges.
