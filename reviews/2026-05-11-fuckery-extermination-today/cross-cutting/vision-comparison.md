# Cross-Cutting Vision Comparison

Status: canonical cross-cutting audit doc.

This compares the original crate-boundary, LM/cache, facade, topology, and
public-maturity vision against the current repository. It does not use the parent
synthesis as central truth; it cites the specs and live code surfaces directly.

## Short Answer

The current repo mostly has the intended boundary names, but not the intended
public maturity. The crate graph is directionally aligned as a knowledge map:
cold core, surface, engine, run, eval, LM/cache, standard vocabulary, GEPA,
agentic, provider/backend, and umbrella crates are named in roughly the intended
places. The failure is that public import paths, default features, examples, and
topology tests make scaffolded or partial boundaries look like ready library
capability.

That matters because the original design is not "many crates compile." It is:

> A Rust optimizer is a configured value that drives a typed run graph by
> proposing changes to artifacts, requesting assessments, interpreting evidence
> through preference relations, and maintaining live populations, while the
> engine provides budgeted, observable, capability-scoped execution.

Evidence: `docs/specs/initial_library.md:4749-4759`.

## Original Cross-Cutting Vision

### Crate Boundaries Are Refusals

- ideal contract: cold core must not know GEPA, surfaces, LMs, workspace, stores,
  or scalar-only assumptions; GEPA is one optimizer, not the engine
  (`docs/specs/initial_library.md:406-443`). `leaven-run` owns product-builder
  ergonomics and lowering without making the umbrella crate an implementation
  bucket
  (`docs/specs/leaven_v0_2_1b_corrected_crate_topology_lib_rs.md:299-314`).
- current reality: the root workspace has the broad crate set
  (`Cargo.toml:3-64`), and the topology contract enforces exact crate dependency
  edges, including `leaven-gepa -> leaven-lm` but not provider crates or
  `leaven-lm-cache` (`crates/leaven/tests/topology_contract.rs:272-291`).
- blocker/gap: topology checks membership and dependencies, not whether public
  crates have behavior. It even phrases `src/lib.rs` presence as a skeleton
  requirement (`crates/leaven/tests/topology_contract.rs:427-433`).
- user impact: the graph can look complete while still allowing false public
  paths.
- correction direction: treat crate graph as a boundary ledger and add a
  public-maturity ledger above it.
- required proof/tests: topology tests must reject scaffold leakage, orphan
  directories, default-feature placeholders, and unallowlisted public unit
  structs, not only dependency drift.

### The Umbrella Is An Import Experience

- ideal contract: Tier 1 users should see a short builder path and not learn
  internal traits (`docs/specs/initial_library.md:451-468`). The public/private
  surface doc says ordinary GEPA users should not learn actors, graph scopes,
  evaluation request templates, split permissions, visibility policy, or run
  graph internals (`docs/specs/gepa_public_private_surface.md:20-47`).
- current reality: `leaven` defaults enable `std`, `derive`, and `gepa`
  (`crates/leaven/Cargo.toml:38-42`). `leaven::prelude` exports engine-author
  types such as `RunContext`, `RunGraphView`, `TrustPolicy`, `Evaluator`,
  `Proposer`, `Renderer`, and `Materializer`
  (`crates/leaven/src/prelude.rs:3-25`), then also exports derive, GEPA, std,
  agentic, and LM-cache preludes (`crates/leaven/src/prelude.rs:27-49`).
- blocker/gap: the ordinary facade collapses Tier 1, Tier 2, Tier 3, scaffold,
  and implementation-wrapper surfaces into one import path.
- user impact: a user trying to run an optimizer is taught too many internals and
  exposed to non-working derive macros.
- correction direction: split ordinary, advanced, and scaffold/test-support
  imports. Default `leaven::prelude` should be boring and product-facing.
- required proof/tests: default import compile test and export ledger for
  `leaven`, `leaven::prelude`, `leaven-std::prelude`, and feature-gated modules.

### GEPA Is One Optimizer, With Swappable Slots

- ideal contract: GEPA has parent selection, part selection, batch sampling,
  reflection/proposal, acceptance, validation, population/frontier, and merge
  scheduling slots (`docs/specs/initial_library.md:443`,
  `docs/specs/initial_library.md:470-485`). The GEPA spec makes those builder
  requirements explicit (`docs/specs/gepa_optimizer_surface.md:271-304`).
- current reality: live GEPA exports public modules and placeholders
  (`crates/leaven-gepa/src/lib.rs:3-35`). Its current proposer seam is
  `SurfaceProposer`, which only receives artifact, surface, and part
  (`crates/leaven-gepa/src/proposer.rs:6-19`). `ReflectiveMutation` is a
  deterministic fixed-edit fixture (`crates/leaven-gepa/src/proposer.rs:21-47`).
- blocker/gap: the live reflection path does not carry the inputs the spec
  requires: parent candidate, selected surface part/view, assessment IDs,
  casewise evidence, attribution evidence, lineage, objective/background, and
  LM-rendered input (`docs/specs/gepa_optimizer_surface.md:447-483`).
- user impact: public GEPA can appear present while not proving real reflective
  mutation. AIME currently wires `ReflectiveMutation` to a hard-coded prompt edit
  (`examples/p8_aime_gepa/src/main.rs:80-99`).
- correction direction: keep the deterministic proposer only as a fixture. Make
  production GEPA names wait for the real async reflective proposer over
  Leaven LM/agent capabilities and scoped evidence.
- required proof/tests: GEPA law/example tests from the spec, especially
  mock-LM reflection, split policy, typed parse errors, and product scenario
  tests (`docs/specs/gepa_optimizer_surface.md:623-652`).

### LM And Cache Are Provider-Neutral Runtime Capabilities, Not GEPA Internals

- ideal contract: `leaven-lm` owns provider-neutral messages, requests,
  responses, usage, continuation, hints, and `Lm`; `leaven-lm-cache` owns response
  cache policy/key/store/backend/`CachedLm`; providers lower into `Lm`; GEPA
  consumes `impl Lm` and not cache stores or provider SDKs
  (`docs/specs/lm_runtime_and_response_cache.md:33-57`).
- current reality: `leaven-lm` is real enough to have request/response modules
  and contract tests (`crates/leaven-lm/src/lib.rs:1-27`,
  `crates/leaven-lm/tests/lm_contract.rs:9-150`). `leaven-lm-cache` is real
  enough to implement cache policy behavior and tests
  (`crates/leaven-lm-cache/src/cached.rs:6-116`,
  `crates/leaven-lm-cache/tests/cache_contract.rs:76-150`). `leaven-lm-openai`
  lowers neutral requests to OpenAI wire JSON and parses responses
  (`crates/leaven-lm-openai/src/client.rs:39-160`).
- blocker/gap: public composition is unsettled. The LM spec shows ordinary code
  manually stacking `CachedLm` (`docs/specs/lm_runtime_and_response_cache.md:17-28`);
  the current product builder has no runtime/cache role configuration; and the
  AIME live path shells out to Python (`examples/p8_aime_gepa/src/main.rs:293-301`).
- user impact: the user cannot swap from mocked LM to OpenAI in the intended
  product path and know that cache/cost/continuation behavior still works.
- correction direction: configure LM/cache by role in the run/runtime layer:
  solver/program runner, reflector/proposer, scorer/judge, and agent runtime.
  Keep `CachedLm` as an advanced wrapper.
- required proof/tests: one end-to-end role-composition test over `leaven-lm`,
  cache, mock/provider mapping, and `leaven-run`; no Python provider bypass in
  product-proof examples.

### Eval, Dataset, Environment, And Execution Stay Separate

- ideal contract: product input is train/validation/test cases, runner/scorer or
  evaluator, and optimizer; lowered eval data is dataset/splits/plan/report;
  execution is engine evaluator calls and graph mutation; environment is optional
  workspace/agent/process substrate (`docs/specs/eval_lowering_detail.md:24-64`).
- current reality: `leaven-run` has public `.train`, `.validation`, `.test`,
  `.runner`, `.score`, `.using`, `.budget`, and `.run` shell
  (`crates/leaven-run/src/builder.rs:54-190`). But runner and scorer are sync
  closures (`crates/leaven-run/src/builder.rs:28-29`), and the evaluator calls
  them synchronously inside an async evaluator
  (`crates/leaven-run/src/evaluator.rs:65-129`).
- blocker/gap: real LM programs, agentic tasks, remote workspaces, and model
  judges need async execution and bounded concurrency. The current sync-only
  shape encourages shell/process bypasses.
- user impact: the product builder looks like the right facade but cannot host
  the main runtime cases cleanly.
- correction direction: hard-cut `leaven-run` runner/scorer to async-capable
  contracts with rich `ScoreContext`, typed outputs/errors, role runtime/cache,
  and bounded concurrency.
- required proof/tests: product builder lowering tests from eval spec:
  stable split IDs, duplicate case rejection, disjoint default, split-use trust
  lowering, final-test-only default, and report truth
  (`docs/specs/eval_lowering_detail.md:650-721`).

### Public Maturity Is A Separate Gate From "Real Code Exists"

- ideal contract: public names are real contracts, private scaffolding,
  test-support public, explicit scaffold features, or explicit fixtures. They
  must not be production-looking public capability with no behavior. The audit
  conventions call scaffolding a finding when it appears in ordinary public paths
  or examples as proof of real capability
  (`reviews/2026-05-11-fuckery-extermination-today/auditing-conventions.md:49-59`).
- current reality: the tree has multiple maturity classes:
  - real: `leaven-lm` contract tests (`crates/leaven-lm/tests/lm_contract.rs:9-150`);
  - real but poorly placed publicly: `CachedLm`
    (`crates/leaven-lm-cache/src/cached.rs:6-116`);
  - mixed: `leaven-std` re-exports broad standard modules
    (`crates/leaven-std/src/lib.rs:3-60`);
  - placeholder: one-line provider/backend structs
    (`crates/leaven-lm-anthropic/src/client.rs:1`,
    `crates/leaven-workspace-docker/src/factory.rs:1`);
  - non-working default scaffold: derive macros
    (`crates/leaven-derive/src/lib.rs:9-33`,
    `crates/leaven-derive/src/unimplemented.rs:3-8`);
  - orphan stale directory: `crates/leaven-dsrs` has files but no manifest/lib
    root.
- blocker/gap: current tests and imports do not enforce these categories.
- user impact: scaffolding and product contracts are visually indistinguishable
  unless a human audits line by line.
- correction direction: create a durable public-maturity gate and then hard-cut
  all default-facing paths to names that pass it.
- required proof/tests: public export ledger plus generated checks for stubs,
  skeleton descriptions, feature maturity, and example proof classification.

## Current Reality Summary

| Area | Ideal | Current | Gap | Required proof |
| --- | --- | --- | --- | --- |
| Crate graph | boundary ledger | broad workspace and edge tests | maturity not tested | topology plus public-maturity gate |
| Umbrella | import experience | default exposes std/derive/gepa and internals | ordinary path too broad | default import compile/export tests |
| GEPA | real optimizer slots | fixed reflection fixture and placeholders | no evidence-aware reflection | GEPA law/product scenario tests |
| LM/cache | provider-neutral runtime roles | real crates, manual wrapper story | no role composition | solver/reflector cache role test |
| Providers/backends | feature means usable adapter | many one-line public structs | feature names overclaim | trait-impl and mapping/law tests |
| Examples | product proof separated from demos | coverage runs proxy examples | green can mean wrong proof | example classification gate |

## Bottom Line

The repo should not be judged by whether the crate names exist. It should be
judged by whether `leaven::prelude::*`, default features, product examples, and
topology tests tell the truth. Today they do not. The fix is not broad
backward-compatible layering; it is a hard cutover to honest public maturity:
remove false public names, make scaffold status explicit, and route product
proof through the LM/cache/GEPA/run surfaces Leaven already claims.
