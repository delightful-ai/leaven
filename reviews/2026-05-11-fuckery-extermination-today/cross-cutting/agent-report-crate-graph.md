# First-Party Cross-Cutting Crate Graph, Stub, and Topology Audit

Date: 2026-05-11

Auditor: Codex, first-party static audit pass

Scope: cross-cutting crate graph, public exports, placeholder/stub inventory,
and topology. I checked the crate manifests and `src/lib.rs` roots under
`crates/*`, examples, scripts, topology specs, and topology contract tests. I
did not run broad verification for this report; this is a documentation write
from static evidence and prior read-only inspection.

This report is intentionally direct. The issue is not just that some code is
unfinished. The issue is that unfinished or proxy behavior is often exposed
through names, features, examples, or topology docs that look like real library
capability. That is the pattern this audit records.

## How To Use This Report

Treat every finding below as a product-contract bug unless it is explicitly
marked as a lower-severity documentation/topology problem. The correction
direction is not a compatibility plan. Leaven's repo contract says hard cutover:
remove false public paths, rename fixtures honestly, or implement the real
surface.

For implementation, start with the high-severity public-path issues:

1. Remove or make honest public defaults that expose non-working derive,
   provider, backend, and standard-library surfaces.
2. Rename or remove GEPA fixtures and placeholder strategy slots before they
   contaminate more examples.
3. Extend the topology contract so the test suite refuses orphan crates and
   public stubs, not just dependency drift.
4. Cut over `leaven-run` runner/scorer to async so real LM and agent execution
   does not require shell escape hatches.

## Findings

### X-001: `leaven-dsrs` Exists As An Orphaned Non-Crate

- `id`: X-001
- `severity`: high
- `surface`: crate graph / topology docs
- `evidence`:
  - `docs/specs/leaven_v0_2_1b_corrected_crate_topology_lib_rs.md:132` lists `leaven-dsrs/` in the crate layout.
  - `docs/specs/leaven_v0_2_1b_corrected_crate_topology_lib_rs.md:210` lists `"crates/leaven-dsrs"` as a workspace member in the topology spec.
  - `docs/specs/leaven_v0_2_1b_corrected_crate_topology_lib_rs.md:540` defines `leaven-dsrs -> [...]` dependency edges.
  - `Cargo.toml:3` through `Cargo.toml:64` lists actual workspace members and does not include `crates/leaven-dsrs`.
  - `crates/leaven/tests/topology_contract.rs:5` through `crates/leaven/tests/topology_contract.rs:66` lists expected workspace members and does not include `crates/leaven-dsrs`.
  - `crates/leaven-dsrs/src/artifact.rs:1` defines only `pub struct DsrsProgramArtifact;`.
  - `crates/leaven-dsrs/src/artifact.rs:3` defines only `pub struct DsrsProgramChange;`.
  - `crates/leaven-dsrs/src/bridge.rs:1` defines only `pub struct DsrsSignatureBridge;`.
  - `crates/leaven-dsrs/src/evaluator.rs:1` defines only `pub struct DsrsEvaluator;`.
  - `crates/leaven-dsrs/src/surface.rs:1` defines only `pub struct DsrsProgramSurface;`.
- `promised behavior`: The topology spec presents `leaven-dsrs` as a domain
  adapter crate for DSRS program artifacts, surfaces, evaluators, and LM
  integration.
- `actual behavior`: The directory exists but is not a Cargo workspace member,
  has no `Cargo.toml`, has no `src/lib.rs`, is not checked by the topology
  contract, and contains only five empty public structs.
- `why it matters`: Future DSRS/GEPA integration work can route toward a crate
  that is not compiled, not tested, and not actually a crate. This is exactly
  the proxy failure the review tree is meant to catch: a spec-visible boundary
  looks available, but the live repo cannot exercise it.
- `correction direction`: Hard-cut one way. Either delete the orphan directory
  and remove stale topology/spec references, or reintroduce `leaven-dsrs` as a
  real workspace crate with a manifest, lib root, implemented public contract,
  tests, and topology-contract membership.

### X-002: Default Umbrella `derive` Feature Exposes Compile-Error Macros

- `id`: X-002
- `severity`: high
- `surface`: umbrella public API
- `evidence`:
  - `crates/leaven/Cargo.toml:39` includes `derive` in the default feature set.
  - `crates/leaven/src/lib.rs:44` through `crates/leaven/src/lib.rs:48`
    re-export derive macros when the feature is enabled.
  - `crates/leaven/src/prelude.rs:27` through `crates/leaven/src/prelude.rs:31`
    re-export derive macros into the common prelude.
  - `crates/leaven-derive/src/lib.rs:9` through
    `crates/leaven-derive/src/lib.rs:33` defines public derives for `Artifact`,
    `ContentAddressed`, and `EditSurface`.
  - `crates/leaven-derive/src/unimplemented.rs:3` through
    `crates/leaven-derive/src/unimplemented.rs:8` makes every derive expand to
    `compile_error!`.
- `promised behavior`: A user who depends on default `leaven` gets working
  derive ergonomics for core artifact and surface traits.
- `actual behavior`: The default feature exposes derive macros that cannot be
  used. Any attempt to use the public macro path intentionally fails at compile
  time.
- `why it matters`: This violates the default import-experience contract.
  Reserved derive names are acceptable as explicit scaffolding, but not as a
  default public user path that looks like core ergonomics.
- `correction direction`: Remove `derive` from `leaven` defaults and prelude
  exports until the derive contract lands, or implement the derive macros fully
  with tests. Do not keep compile-error macros on the ordinary default path.

### X-003: `leaven-std` Re-Exports Placeholder Implementations As Standard Library

- `id`: X-003
- `severity`: high
- `surface`: standard library facade / public exports
- `evidence`:
  - `crates/leaven/Cargo.toml:39` enables `std` by default.
  - `crates/leaven-std/src/lib.rs:3` through
    `crates/leaven-std/src/lib.rs:45` re-export artifacts, evidence,
    preferences, populations, renderers, and surfaces as standard modules.
  - `crates/leaven-artifacts/src/lib.rs:1` calls the crate a skeleton.
  - `crates/leaven-artifacts/src/lib.rs:3` through
    `crates/leaven-artifacts/src/lib.rs:22` exports empty artifact/surface
    structs such as `DirArtifact`, `PartMapArtifact`, and `TextArtifact`.
  - `crates/leaven-render/src/lib.rs:1` calls the crate a skeleton.
  - `crates/leaven-render/src/materializer.rs:1` through
    `crates/leaven-render/src/materializer.rs:5` exports empty materializer
    structs.
  - `crates/leaven-population/src/beam.rs:1`,
    `crates/leaven-population/src/map_elites.rs:1`,
    `crates/leaven-population/src/novelty.rs:1`, and
    `crates/leaven-population/src/plackett_luce.rs:1` expose empty population
    strategy names.
- `promised behavior`: `leaven-std` is the curated standard library of usable
  Leaven building blocks.
- `actual behavior`: The facade mixes real pieces with empty public unit
  structs and skeleton crates. Those names are exposed as standard imports, not
  quarantined scaffolding.
- `why it matters`: Users and implementors can reasonably import `TextArtifact`,
  `ReflectionPromptRenderer`, `BeamPopulation`, or `MapElites` expecting
  behavior. Instead, many of these names are inert markers. That creates false
  confidence and gives future code attractive but hollow anchors.
- `correction direction`: Remove placeholder exports from `leaven-std` and its
  prelude, or gate them behind an explicitly named scaffolding/test feature.
  Standard exports should be behavior-bearing or absent.

### X-004: Provider And Backend Features Expose Empty Adapter Types

- `id`: X-004
- `severity`: high
- `surface`: provider/backend crates
- `evidence`:
  - `crates/leaven/Cargo.toml:51` through `crates/leaven/Cargo.toml:55` exposes
    optional workspace and LM provider features.
  - `crates/leaven/src/lib.rs:68` through `crates/leaven/src/lib.rs:75`
    re-export LM cache/OpenAI/Anthropic provider crates when features are
    enabled.
  - `crates/leaven-lm-anthropic/src/client.rs:1` defines only
    `pub struct AnthropicLm;`.
  - `crates/leaven-lm-local/src/client.rs:1` defines only `pub struct LocalLm;`.
  - `crates/leaven-workspace-docker/src/factory.rs:1` defines only
    `pub struct DockerWorkspaceFactory;`.
  - `crates/leaven-workspace-e2b/src/factory.rs:1` defines only
    `pub struct E2bWorkspaceFactory;`.
  - `crates/leaven-workspace-firecracker/src/factory.rs:1` defines only
    `pub struct FirecrackerWorkspaceFactory;`.
  - `crates/leaven-workspace-git/src/factory.rs:1` defines only
    `pub struct GitWorkspaceFactory;`.
  - `crates/leaven-workspace-k8s/src/factory.rs:1` defines only
    `pub struct K8sWorkspaceFactory;`.
  - `crates/leaven-store-object/src/store.rs:1` defines only
    `pub struct ObjectStore;`.
  - `crates/leaven-store-sqlite/src/store.rs:1` defines only
    `pub struct SqliteStore;`.
- `promised behavior`: Enabling provider/backend features exposes concrete LM,
  workspace, object-store, and SQLite integrations.
- `actual behavior`: Several feature-gated crates expose only empty public
  structs with no trait implementations, constructors, errors, or behavior.
- `why it matters`: Feature names communicate availability. A user can enable
  `lm-anthropic`, `workspace-docker`, `workspace-e2b`, or `store-sqlite` and
  receive only inert types. This makes the public crate graph look broader than
  the actual implementation.
- `correction direction`: Remove these feature exports until real adapters
  exist, or implement the adapters against the owning capability traits with
  tests. If a name must remain reserved, put it behind an explicit
  scaffolding feature and document it as non-production.

### X-005: GEPA Public Strategy Surface Contains Fixed Fixtures And Placeholders

- `id`: X-005
- `severity`: blocker
- `surface`: GEPA public API / examples
- `evidence`:
  - `crates/leaven-gepa/src/proposer.rs:7` through
    `crates/leaven-gepa/src/proposer.rs:18` defines `SurfaceProposer`, which
    sees only artifact, surface, and selected part.
  - `crates/leaven-gepa/src/proposer.rs:21` calls `ReflectiveMutation` a
    deterministic fixture.
  - `crates/leaven-gepa/src/proposer.rs:27` through
    `crates/leaven-gepa/src/proposer.rs:32` stores one edit.
  - `crates/leaven-gepa/src/proposer.rs:40` through
    `crates/leaven-gepa/src/proposer.rs:47` always returns that stored edit.
  - `crates/leaven-gepa/src/proposer.rs:50` through
    `crates/leaven-gepa/src/proposer.rs:56` exposes
    `ReflectiveMutationConfig` and `SystemAwareMerge` placeholders.
  - `crates/leaven-gepa/src/optimizer.rs:536` through
    `crates/leaven-gepa/src/optimizer.rs:563` uses the GEPA-local
    `SurfaceProposer` path for candidate proposals.
  - `crates/leaven-gepa/src/optimizer.rs:716` through
    `crates/leaven-gepa/src/optimizer.rs:722` exposes `GepaConfig` and
    `MergeScheduler` placeholders.
  - `examples/p8_aime_gepa/src/main.rs:91` through
    `examples/p8_aime_gepa/src/main.rs:93` wires the AIME example reflector to
    a hard-coded optimized prompt edit.
- `promised behavior`: GEPA's public API exposes real reflective mutation and
  strategy slots capable of using traces, feedback, selected cases, graph
  context, LM calls, or agentic reflection.
- `actual behavior`: The central reflective mutation type is a fixed-edit
  fixture, several strategy names are placeholders, and the proposer request
  shape cannot even name the evidence/trace context reflection needs.
- `why it matters`: This is the worst false-positive path in the current tree.
  Examples can show GEPA-like score movement while the library has not proven
  real reflection. It also blocks GEPA customizers because the strategy seam is
  narrower than the intended behavior.
- `correction direction`: Rename the fixed fixture to something honest such as
  `FixedEditProposer` and move it to tests/examples, or remove it from public
  API. Reserve `ReflectiveMutation` for a real async reflector request that
  carries selected candidate, selected part, current part text, scored
  trace/evidence, objective/background, budget, and scoped graph/evidence
  access. Remove or implement placeholder strategy slots.

### X-006: GEPA Cache Dependency Direction Is Contradictory In The Spec

- `id`: X-006
- `severity`: medium
- `surface`: topology docs
- `evidence`:
  - `docs/specs/gepa_optimizer_surface.md:174` through
    `docs/specs/gepa_optimizer_surface.md:188` says `leaven-gepa` may depend on
    `leaven-lm-cache`.
  - `docs/specs/gepa_optimizer_surface.md:190` through
    `docs/specs/gepa_optimizer_surface.md:197` says `leaven-gepa` must not
    depend on `leaven-lm-cache`.
  - `crates/leaven-gepa/Cargo.toml:13` through
    `crates/leaven-gepa/Cargo.toml:23` shows the live crate does not depend on
    `leaven-lm-cache`.
  - `crates/leaven/tests/topology_contract.rs:272` through
    `crates/leaven/tests/topology_contract.rs:283` mirrors the live dependency
    set and omits `leaven-lm-cache`.
- `promised behavior`: The GEPA topology spec gives implementors a clear
  dependency boundary for LM and cache composition.
- `actual behavior`: The same spec both allows and forbids the cache crate as a
  GEPA dependency. The live manifest and topology contract choose omission, but
  the governing doc is contradictory.
- `why it matters`: Cache ownership is already a public-surface smell in this
  audit tree. If GEPA authors follow the wrong half of the spec, cache policy
  may leak into GEPA strategy code or be duplicated there.
- `correction direction`: Decide one boundary and update the spec, manifest,
  and topology contract together. The likely clean shape is that GEPA speaks
  provider-neutral `leaven-lm` request/response vocabulary while cache policy
  is configured above GEPA through run/runtime composition.

### X-007: Topology Contract Proves Skeleton Presence, Not Stub Absence

- `id`: X-007
- `severity`: medium
- `surface`: topology contract tests
- `evidence`:
  - `crates/leaven/tests/topology_contract.rs:421` through
    `crates/leaven/tests/topology_contract.rs:443` asserts workspace members,
    manifests, and `src/lib.rs` files exist.
  - `crates/leaven/tests/topology_contract.rs:427` through
    `crates/leaven/tests/topology_contract.rs:433` explicitly says each crate
    must expose a `src/lib.rs skeleton`.
  - `crates/leaven/tests/topology_contract.rs:446` through
    `crates/leaven/tests/topology_contract.rs:458` checks exact dependency
    edges.
  - `crates/leaven/tests/topology_contract.rs:461` through
    `crates/leaven/tests/topology_contract.rs:477` checks only a narrow
    cold-core leak class.
  - `crates/leaven/tests/topology_contract.rs:479` through
    `crates/leaven/tests/topology_contract.rs:505` checks Codex protocol
    leaf-only constraints and umbrella Codex absence.
- `promised behavior`: The topology contract should protect crate-boundary
  truth.
- `actual behavior`: The contract is good at exact membership and dependency
  drift, but it does not reject extra unregistered `crates/*` directories,
  skeleton package descriptions, empty public unit structs, public placeholder
  exports, or standard/umbrella re-exports of scaffolding. It even encodes
  skeleton presence as a positive requirement.
- `why it matters`: The test suite can pass while the public graph remains full
  of fake names. That makes the topology contract a partial guardrail, not an
  audit of whether crate boundaries are honest.
- `correction direction`: Extend topology checks to:
  - compare every `crates/*` directory against workspace membership;
  - reject `description = "Leaven crate skeleton ..."` in workspace crates
    unless the crate is explicitly in a scaffold allowlist;
  - reject public unit structs in production modules unless allowlisted;
  - reject umbrella/std re-exports of scaffold crates;
  - keep the existing exact dependency-edge checks.

### X-008: Coverage Gate Runs Proxy Examples As If They Were Product Proof

- `id`: X-008
- `severity`: medium
- `surface`: verification scripts / examples
- `evidence`:
  - `scripts/coverage-gate.py:13` through `scripts/coverage-gate.py:24` lists
    all milestone examples, including `p8_aime_gepa`, as packages to execute
    for coverage.
  - `scripts/coverage-gate.py:26` through `scripts/coverage-gate.py:28` marks
    only `p5_evoskill_iteration` as a live package excluded from normal
    workspace coverage.
  - `examples/p8_aime_gepa/README.md:9` says the default path is deterministic
    and uses a scripted solver.
  - `examples/p8_aime_gepa/README.md:33` says the deterministic path is not
    evidence of live AIME improvement.
  - `examples/p8_aime_gepa/src/main.rs:271` through
    `examples/p8_aime_gepa/src/main.rs:291` implements the default scripted
    solver path.
  - `examples/p8_aime_gepa/src/main.rs:293` through
    `examples/p8_aime_gepa/src/main.rs:315` implements the live path by
    shelling out to Python.
- `promised behavior`: Coverage and example execution should give confidence
  that public product paths work.
- `actual behavior`: The coverage gate executes proxy examples, including a
  deterministic AIME path that the README explicitly says is not evidence of
  live improvement. The live solver path is outside the Leaven LM/runtime/cache
  stack.
- `why it matters`: Coverage can ratify nearby proxies as product proof. This
  directly repeats the failure mode described in the user-message archive:
  "implemented" examples that quietly substitute the hard part.
- `correction direction`: Split demo/proxy coverage from product capability
  gates. Either remove proxy examples from proof-bearing coverage or add a
  separate explicit "not product proof" classification in tooling. Live paths
  should exercise Leaven-owned LM/runtime/cache APIs.

### X-009: Several Crates Leak Internal Module Layout Through `pub mod`

- `id`: X-009
- `severity`: medium
- `surface`: public module layout
- `evidence`:
  - `crates/leaven-gepa/src/lib.rs:3` through
    `crates/leaven-gepa/src/lib.rs:8` exposes `gate`, `optimizer`,
    `part_selector`, `proposer`, `selector`, and `validation` as public
    modules.
  - `crates/leaven-workspace/src/lib.rs:3` through
    `crates/leaven-workspace/src/lib.rs:10` exposes internal modules such as
    `command`, `config`, `factory`, `policy`, `view`, and `workspace`.
  - `crates/leaven-agent-command/src/lib.rs:3` through
    `crates/leaven-agent-command/src/lib.rs:6` exposes config/error/parser/runtime
    modules directly.
  - `crates/leaven-agent-codex-app-server/src/lib.rs:7` through
    `crates/leaven-agent-codex-app-server/src/lib.rs:17` exposes config, error,
    runtime, and transport modules directly while also providing curated
    re-exports.
- `promised behavior`: Crate roots are maps and curated public contracts.
- `actual behavior`: Several crate roots expose file-layout modules as public
  paths in addition to curated root-level exports.
- `why it matters`: Downstream users can depend on internal layout paths that
  should remain private design freedom. This widens the public API more than
  intended and makes future hard cutovers harder.
- `correction direction`: Make modules private by default and expose only
  deliberate root/prelude contracts. Keep `pub mod` only when the module path
  itself is an intended stable namespace.

### X-010: High-Level `leaven-run` Runner And Scorer Are Sync-Only

- `id`: X-010
- `severity`: high
- `surface`: high-level optimize builder
- `evidence`:
  - `crates/leaven-run/src/builder.rs:28` defines `Runner<A, C>` as
    `Arc<dyn Fn(&A, &C) -> RunOutput + Send + Sync>`.
  - `crates/leaven-run/src/builder.rs:29` defines `Scorer<A, C>` as a
    synchronous function.
  - `crates/leaven-run/src/builder.rs:136` through
    `crates/leaven-run/src/builder.rs:143` accepts only sync runner closures.
  - `crates/leaven-run/src/builder.rs:146` through
    `crates/leaven-run/src/builder.rs:153` accepts only sync scorer closures.
  - `crates/leaven-run/src/evaluator.rs:97` through
    `crates/leaven-run/src/evaluator.rs:102` calls runner and scorer
    synchronously inside an async evaluator.
  - `examples/p8_aime_gepa/src/main.rs:293` through
    `examples/p8_aime_gepa/src/main.rs:301` shells out to Python for a live
    OpenAI solver instead of using an async Leaven LM path.
- `promised behavior`: The high-level builder can run real LM/agent programs
  through the same optimize/train/validation/test surface.
- `actual behavior`: The public runner/scorer hooks are sync-only, so real LM
  calls, agent sessions, remote workspaces, and model judges need hidden
  runtimes, `block_on`, shelling out, or a bypass.
- `why it matters`: Ordinary users should not need to work around the public
  API to run the main optimizer-library use case. Sync-only execution also
  blocks honest bounded concurrency for benchmark runs.
- `correction direction`: Hard-cut to async runner/scorer/evaluator closures
  with explicit bounded concurrency. Preserve simple sync ergonomics only if
  they lower into the same async path without creating a second semantic lane.

### X-011: OpenAI Provider Constructor Accepts A Default Model It Does Not Own

- `id`: X-011
- `severity`: medium
- `surface`: LM provider public API
- `evidence`:
  - `crates/leaven-lm-openai/src/client.rs:27` through
    `crates/leaven-lm-openai/src/client.rs:35` documents and accepts
    `from_env(default_model)`.
  - `crates/leaven-lm-openai/src/client.rs:35` names the parameter
    `_default_model`.
  - `crates/leaven-lm-openai/src/client.rs:44` through
    `crates/leaven-lm-openai/src/client.rs:47` lowers the model from
    `LmRequest`, not provider state.
  - `crates/leaven-lm-openai/src/client.rs:129` through
    `crates/leaven-lm-openai/src/client.rs:133` fingerprints only the provider
    version string and base URL, not a default model.
- `promised behavior`: The constructor suggests `OpenAiLm` owns a default model
  that contributes to ergonomic request construction or fingerprint stability.
- `actual behavior`: The provider ignores the default model value. Requests
  carry their own model.
- `why it matters`: This is a smaller but corrosive public-contract mismatch.
  Users build the wrong mental model, and provider fingerprints cannot honestly
  include a default model because it is not stored.
- `correction direction`: Either make `OpenAiLm` own a default model and expose
  helpers that use it, or remove the argument from `from_env`.

### X-012: `FakeAgentRuntime` Is Public In The Provider-Neutral Agent Prelude

- `id`: X-012
- `severity`: low
- `surface`: agent runtime public exports
- `evidence`:
  - `docs/specs/agentic_stage_runtime.md:921` through
    `docs/specs/agentic_stage_runtime.md:924` explicitly calls for a
    deterministic fake runtime for tests.
  - `docs/specs/agentic_stage_runtime.md:934` says to start with the contract
    and a fake runtime.
  - `crates/leaven-agent/src/lib.rs:9` through
    `crates/leaven-agent/src/lib.rs:11` re-export `FakeAgentAction` and
    `FakeAgentRuntime` at the crate root.
  - `crates/leaven-agent/src/lib.rs:23` through
    `crates/leaven-agent/src/lib.rs:31` also re-export the fake runtime in the
    prelude.
  - `crates/leaven-agent/src/fake.rs:1` documents it as deterministic runtime
    for contract tests and examples.
- `promised behavior`: The fake runtime is a testing/example utility for
  proving the agent runtime contract before provider adapters land.
- `actual behavior`: The fake runtime is part of the ordinary public prelude
  for the provider-neutral runtime crate.
- `why it matters`: This is not as severe as the GEPA fixture because the fake
  is honestly named. Still, putting it in the common prelude can normalize fake
  runtime paths as ordinary user API and blur the line between tests/examples
  and production providers.
- `correction direction`: Keep the fake runtime public only if it is explicitly
  a test-support/example utility. Consider moving it out of the common prelude
  or behind a feature with clear naming.

## Concise Crate Inventory Grouped By Boundary

This inventory is intentionally pragmatic. "Real" means behavior-bearing enough
that it has actual code and tests or a clear implemented contract. "Mixed"
means some real behavior exists but public stubs or placeholder exports remain.
"Placeholder" means the crate mostly exports names, not behavior.

### Core / Cold Substrate

- `leaven-kernel`: real universal IDs, cost, finite floats, metadata,
  fingerprints, time, and error-record primitives.
- `leaven-core`: real cold optimizer algebra: artifacts, evidence,
  proposals, evaluations, preferences, and problem vocabulary.
- `leaven-surface`: real edit/read surface vocabulary and path/part selection
  primitives.
- `leaven-store`: real storage capability traits; intentionally capability
  oriented, not backend behavior.
- `leaven-engine`: real run graph, run context, budget/cache/trust/event/stage
  execution substrate.

### Product/User Surfaces

- `leaven`: mixed. It is a real umbrella crate, but default features and
  re-exports expose non-working derive and broad public internals.
- `leaven-run`: mixed. It provides a real high-level builder and split/report
  lowering, but runner/scorer are sync-only and force real LM/agent paths into
  bypasses.
- `leaven-eval`: mostly real lowered eval data/split/report vocabulary.
- `leaven-lm`: real provider-neutral request/response/message/model
  vocabulary.
- `leaven-lm-cache`: real cache wrapper/store/policy pieces, but the public
  wrapper shape is already called out elsewhere as an API smell.
- `leaven-lm-openai`: partially real OpenAI Responses lowering/client, with a
  public constructor mismatch around default model ownership.
- `leaven-lm-mock`: real test/mock LM.

### Agentic / Workspace Stack

- `leaven-agent`: mostly real provider-neutral runtime contract, transcript,
  session, and fake runtime. Fake runtime export/prelude placement is a minor
  public-surface smell.
- `leaven-agent-command`: real command-backed runtime substrate.
- `leaven-agent-codex-cli`: real Codex CLI runtime adapter.
- `leaven-agent-codex-app-server`: real feature-gated Codex app-server adapter
  surface; current topology around protocol leaf-ness is guarded.
- `leaven-agent-codex`: facade crate, intentionally optional/provider-family
  shaped.
- `leaven-agentic`: real agentic proposer/evaluator/parser/repair substrate.
- `leaven-agentic-skill`: real skill-specific agentic adapter surface.
- `leaven-artifact-skill`: real skill artifact/change/surface model.
- `leaven-workspace`: real backend-neutral workspace traits/path/view/policy.
- `leaven-workspace-local`: real local backend.

### Standard Vocabularies And Strategy Crates

- `leaven-evidence`: mixed. Many evidence shapes are real, but the manifest and
  root still identify as skeleton and should be cleaned up.
- `leaven-population`: mixed. `KeepBest`, `ParetoFrontier`, and tournament
  pieces carry behavior, but several public population names are empty.
- `leaven-preference`: mixed. Scalar preference has behavior; pareto/ranking
  names are mostly empty markers.
- `leaven-std`: mixed and risky because it re-exports both real and placeholder
  pieces as standard library.
- `leaven-gepa`: mixed and high-risk. The optimizer skeleton has real graph
  integration, but the reflective/proposer seam and strategy slots are not yet
  honest enough for real GEPA.

### Placeholder / Mostly Empty Crates

These should either be implemented, deleted, or moved behind explicit
scaffolding features:

- `leaven-artifacts`
- `leaven-artifact-git`
- `leaven-artifact-jj`
- `leaven-render`
- `leaven-mipro`
- `leaven-textgrad`
- `leaven-trace`
- `leaven-cuda`
- `leaven-python`
- `leaven-lm-anthropic`
- `leaven-lm-local`
- `leaven-agent-claude-code`
- `leaven-agent-opencode`
- `leaven-store-object`
- `leaven-store-sqlite`
- `leaven-workspace-docker`
- `leaven-workspace-e2b`
- `leaven-workspace-firecracker`
- `leaven-workspace-git`
- `leaven-workspace-k8s`

### Orphan / Stale Directory

- `crates/leaven-dsrs`: not a workspace crate, no manifest, no lib root, only
  one-line public structs. The spec still mentions it. This should be treated
  as stale or reintroduced properly, not ignored.

### Examples And Scripts

- `examples/p0_graph_skeleton`: real graph mutation smoke, low-level.
- `examples/p1_keep_best`: real scalar keep-best engine path, low-level.
- `examples/p2_pairwise_tournament`: real pairwise/tournament path, still
  deterministic fixture style.
- `examples/p3_gepa_parity`: useful GEPA mechanics smoke, but depends on
  fixed-edit reflection.
- `examples/p4_meta_harness_lite`: useful workspace/materialization smoke, but
  uses `fake_agent_author_harness` as example-local deterministic agent work.
- `examples/p5_evoskill_iteration`: heavier live-ish agentic example over more
  real substrate.
- `examples/p6_optimizer_policy_self_opt`: deterministic policy self-opt
  scenario.
- `examples/p7_self_optimization_kernel`: deterministic self-optimization
  kernel scenario.
- `examples/p8_aime_gepa`: public API AIME path, but current deterministic
  proof and live OpenAI path do not prove real Leaven GEPA reflection or
  Leaven-owned LM/cache integration.
- `scripts/coverage-gate.py`: useful coverage gate, but currently executes
  proxy examples as part of coverage proof without a separate product-proof
  classification.
- `scripts/lint-line-count.py`: scoped production Rust line-count lint.
- `scripts/test-suite-sla.py`: scoped test-suite SLA wrapper.

## Non-Findings

### Codex App-Server Protocol Is Currently Leaf-Bounded

I do not see a current cross-cutting topology violation for the Codex
app-server protocol boundary. `docs/specs/codex_app_server_agent_runtime.md:113`
through `docs/specs/codex_app_server_agent_runtime.md:128` says the concrete
app-server crate owns protocol dependencies, and
`docs/specs/codex_app_server_agent_runtime.md:748` through
`docs/specs/codex_app_server_agent_runtime.md:756` says umbrella Codex features
should not exist yet. The topology contract enforces this at
`crates/leaven/tests/topology_contract.rs:479` through
`crates/leaven/tests/topology_contract.rs:505`.

This is a useful pattern to copy: leaf-only provider protocol deps plus an
explicit umbrella-feature refusal.

## Suggested Immediate Follow-Up Checks

These are not implementation steps; they are narrow audit/proof checks a future
implementor can add before fixing behavior:

1. Add a generated ledger of all public unit structs in non-test code.
2. Add a generated ledger of all crates whose package description contains
   `skeleton`.
3. Add a topology test that every `crates/*` directory is either a workspace
   member or appears in an explicit stale-directory allowlist.
4. Add a public-export ledger for `leaven`, `leaven-std`, and each `prelude`.
5. Classify examples as `product-proof`, `mechanics-smoke`, or `proxy-demo` so
   coverage cannot silently promote proxy examples.

