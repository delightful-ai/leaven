# Map-First AGENTS.md Hierarchy Checkpoint

Status: active checkpoint, not final hierarchy.
Date: 2026-05-11.

This note preserves the useful findings from the first subagent wave so the
remaining scouts can keep reading deeply without blocking the next integration
pass. It is a dated planning note, not durable repo law. Promote stable rules
from here into the nearest owning `AGENTS.md`, spec, test, or crate doc.

## Operating Direction

The first hierarchy pass should optimize for map density before invariant
density.

Primary question for each future agent:

```text
Given concept X, where do I start, which crate owns it, what neighboring crate
must not own it, and what proof anchor shows the current contract?
```

Useful `AGENTS.md` lines should route work, not merely describe folders.
Decision cards and `NEVER:` lines are still useful, but only after the map shows
where a future agent is likely to put a concept in the wrong place.

## Reports Received

Received:

- beads-rs inspiration scout
- substrate/execution scout
- algorithms/vocabulary/examples scout
- spec atlas scout
- adversarial review scout
- interfaces/adapters scout

Still pending at the time this checkpoint was first written:

- spec atlas scout
- interfaces/adapters scout
- adversarial review scout

These have now arrived and are integrated below. Do not force future scouts to
downshift into a shorter report just to unblock drafting; add dated checkpoints
instead.

## Spec Atlas

Document authority:

- `docs/specs/initial_library.md` is the active governing product and
  architecture spec.
- `docs/specs/guiding_principles.md` is the active requirements and taste
  filter, not crate layout.
- `docs/specs/leaven_v0_2_1b_corrected_crate_topology_lib_rs.md` is the active
  topology intent document, but its manifest/tree details lag current code.
- `docs/testing/README.md` is the active proof contract.
- `docs/AGENTSMD_INFO.md` is the AGENTS rollout rubric, not product
  architecture.
- Companion specs own narrower surfaces: agentic runtime, skill optimization,
  Codex runtimes, LM/runtime/cache, milestone behavior.
- `docs/plans/` are dated execution notes.
- `docs/specs/first_two_subsystems.md` is superseded historical context.
- `docs/philosophy/*` are decision filters; convert them into specs, tests,
  crate docs, or AGENTS rules before treating them as operational.

Spec-derived concept atlas:

- `Amount`, `Cost`, IDs, metadata, fingerprints, and durable errors:
  `leaven-kernel`.
- `Artifact`, `Change`, `ArtifactIdentity`, and `CacheIdentity`: `leaven-core`.
- `EditSurface`, parts, addresses, and surface fingerprints: `leaven-surface`,
  with artifact-specific concrete surfaces in artifact crates.
- `Candidate`, `RunGraph`, graph views, lineage, `RunContext`, events, trust,
  cache, and engine loop: `leaven-engine`.
- `Proposal`, `ProposalEffect`, `CausalInputs`, and `InfoRef`: cold shape in
  `leaven-core`; graph validation in `leaven-engine`.
- `EvaluationSet`, `EvaluationRequest`, and `Assessment`: cold shape in
  `leaven-core`; resolution/execution in `leaven-engine`; lowered product data
  in `leaven-eval`.
- Evidence marker and capabilities: marker in `leaven-core`; reusable evidence
  shapes in `leaven-evidence`.
- Preference: result vocabulary in `leaven-core`; relation trait in
  `leaven-engine`; stateless impls in `leaven-preference`; fitted models in
  `leaven-population`.
- Population/frontier/niche: trait in `leaven-engine`; standard impls in
  `leaven-population`; GEPA selectors in `leaven-gepa`.
- Renderer/materializer: traits in `leaven-engine`; reusable impls in
  `leaven-render`; workspace substrate in `leaven-workspace`.
- Workspace and `WorkspacePath`: `leaven-workspace`; concrete backends in
  `leaven-workspace-*`.
- Storage capabilities: `leaven-store`; concrete backends in
  `leaven-store-*`; graph persistence codec remains engine-owned.
- LM runtime/cache/providers: `leaven-lm`, `leaven-lm-cache`, `leaven-lm-*`.
- Agent runtime and agentic stages: `leaven-agent`, `leaven-agent-*`,
  `leaven-agentic*`.
- Optimizer algorithms: `leaven-gepa`, `leaven-mipro`, `leaven-textgrad`,
  `leaven-trace`.
- Product builder and import experience: `leaven-run`; umbrella `leaven`
  re-exports only.

Spec-atlas risks:

- The topology spec is marked pre-implementation and contains stale manifest
  details versus current `Cargo.toml`, including resolver/version drift, P8,
  and `leaven-artifact-skill`.
- `docs/specs/tracing-vision/README.md` names `first_two_subsystems.md` as a
  governing spec even though that file is explicitly superseded.
- `docs/testing/README.md` omits `just milestone-p8` from its command list.
- `docs/specs/philosophy_compliance_cleanup.md` lacks a clear status line.
- Several 2026-05-10 specs are planning specs; verify code/tests before
  promoting their vocabulary into AGENTS files.

## Beads-RS Patterns To Copy

Copy the shape, not the product content.

- Keep root rich as the only guaranteed context, but put local routing in child
  files rather than duplicating the root map.
- Child files should mostly be delta: what this crate or subtree owns, what a
  sibling owns instead, and which proof loop matches the local claim.
- Strong prohibitions should be temptation-based: name the wrong move a future
  agent would plausibly make, then route to the owning crate.
- Seam directories deserve canonical extension paths when work must flow
  through several modules in order.
- Test-root `AGENTS.md` files should describe the proof model and helper stack,
  not just list commands.
- Docs subtrees need an authority ladder so specs, plans, philosophy, testing
  policy, and dated evidence are not treated as equal truth.
- Tooling docs should classify side effects and rerun safety, not just list
  scripts.

Do not copy beads-specific workflow such as `bd prime`, bead IDs in commits,
CRDT/store-ref mental model, daemon/tailnet proof lanes, or legacy/quarantine
language unless Leaven has the same concrete shape.

## Substrate And Execution Map

Immediate child homes recommended by the substrate/execution scout:

- `crates/leaven-kernel/AGENTS.md`
- `crates/leaven-core/AGENTS.md`
- `crates/leaven-surface/AGENTS.md`
- `crates/leaven-engine/AGENTS.md`
- `crates/leaven-run/AGENTS.md`
- `crates/leaven/AGENTS.md`

Concept routes:

- Mechanical IDs, cost, finite numbers, fingerprints, metadata, time, and
  durable errors belong in `leaven-kernel`.
- Artifact identity, apply/cache identity, proposal effect/provenance/batches,
  evaluation request/set vocabulary, evidence marker, preference result, and
  problem associated types belong in `leaven-core`.
- Artifact projections, parts, addresses, views, surface edits, and surface
  fingerprints belong in `leaven-surface`.
- `RunGraph`, graph views, `RunContext`, stage traits, budget ledger, cache,
  trust/read scopes, events, reports, persistence, and the engine loop belong in
  `leaven-engine`.
- Public builder ergonomics, train/validation/test policy, default evaluator or
  store wiring, and result facades belong in `leaven-run`.
- Re-exports, feature gates, preludes, and cross-crate end-to-end/topology tests
  belong in `leaven`.

Route-away lines worth preserving:

- `leaven-kernel` must not learn artifact or optimizer vocabulary.
- `leaven-core` must route parts/projections to `leaven-surface`; graph
  mutation, cache, budgets, trust, and stage traits to `leaven-engine`; reusable
  implementations to standard vocabulary crates.
- `leaven-surface` must not claim decomposition is intrinsic to artifacts.
- `leaven-engine` must route optimizer strategy state to optimizer crates,
  product defaults to `leaven-run`, reusable evidence/preference/population to
  standard crates, and concrete providers/backends to adapter crates.
- `leaven-run` must not become an engine internals or graph shortcut layer.
- `leaven` must stay import shape, not implementation bucket.

Concrete drift found:

- `docs/specs/leaven_v0_2_1b_corrected_crate_topology_lib_rs.md` is older
  than `docs/specs/initial_library.md` and omits later crates/examples.
- The topology spec omits `leaven-artifact-skill` and examples `p5` through
  `p8`; `crates/leaven/tests/topology_contract.rs` includes them.
- The topology spec omits `leaven-surface` from the engine dependency list,
  while the topology contract includes it. The scout did not find direct
  `leaven_surface` source usage in engine, so this is either reserved
  dependency drift or removable dependency drift.
- The topology spec omits `leaven-evidence` from the `leaven-run` dependency
  list, while code and the topology contract include it.
- `docs/testing/README.md` omits some live suites seen in the tree, including
  `crates/leaven-engine/tests/evaluator_registry.rs`,
  `crates/leaven-run/tests/optimize_builder.rs`, and
  `crates/leaven-run/tests/scoring_evaluator.rs`.

## Algorithms, Vocabulary, And Examples Map

Immediate child homes recommended by the algorithms/vocabulary/examples scout:

- `crates/leaven-evidence/AGENTS.md`
- `crates/leaven-population/AGENTS.md`
- `crates/leaven-gepa/AGENTS.md`
- `crates/leaven-eval/AGENTS.md`
- `crates/leaven-artifact-skill/AGENTS.md`
- `crates/leaven-std/AGENTS.md`
- `examples/p5_evoskill_iteration/AGENTS.md`
- `examples/p8_aime_gepa/AGENTS.md`

Defer until behavior lands beyond skeleton names:

- `crates/leaven-artifacts/AGENTS.md`
- `crates/leaven-render/AGENTS.md`
- `crates/leaven-mipro/AGENTS.md`
- `crates/leaven-textgrad/AGENTS.md`
- `crates/leaven-trace/AGENTS.md`

Concept routes:

- `leaven-core` owns the artifact trait vocabulary.
- `leaven-artifacts` owns generic reusable artifacts.
- `leaven-artifact-*` crates own concrete artifact families and their surfaces.
- `leaven-evidence` owns reusable evidence shapes.
- `leaven-eval` owns dataset/split/report vocabulary, not execution.
- `leaven-preference` owns stateless relations.
- `leaven-population` owns stateful archives, frontiers, fitted models, and
  population update state such as `KeepBest`, `ParetoFrontier`, and
  `TournamentPopulation`.
- `leaven-render` owns reusable renderer/materializer names.
- Optimizer crates own algorithm rhythm and strategy state, not engine
  machinery.
- Examples are runnable acceptance tests for public workflows, not homes for
  reusable behavior.

Route-away lines worth preserving:

- Do not put implementation in `leaven-std`; move behavior to the owning crate,
  then re-export it if appropriate.
- Do not put fitted Bradley-Terry or tournament state in `leaven-preference`;
  route that to `leaven-population`.
- Do not put GEPA selectors, gates, reflection policy, or surface lowering in
  `leaven-engine`; route that to `leaven-gepa`.
- Do not put reusable evidence, population, or materializer behavior in
  examples.
- Do not put provider CLI/runtime details in optimizer crates.

## Interfaces And Adapters Map

Immediate child homes recommended by the interfaces/adapters scout:

- `crates/leaven-store/AGENTS.md`
- `crates/leaven-store-file/AGENTS.md`
- `crates/leaven-workspace/AGENTS.md`
- `crates/leaven-workspace-local/AGENTS.md`
- `crates/leaven-lm/AGENTS.md`
- `crates/leaven-lm-cache/AGENTS.md`
- `crates/leaven-lm-openai/AGENTS.md`
- `crates/leaven-agent/AGENTS.md`
- `crates/leaven-agent-command/AGENTS.md`
- `crates/leaven-agent-codex/AGENTS.md`
- `crates/leaven-agent-codex-cli/AGENTS.md`
- `crates/leaven-agent-codex-app-server/AGENTS.md`
- `crates/leaven-agentic/AGENTS.md`
- `crates/leaven-agentic-skill/AGENTS.md`

Family routes:

- `leaven-store` owns `BlobStore`, `EvidenceStore`, and `CheckpointStore`;
  backends own layout/codecs and depend inward. Store must not know `RunGraph`.
- `leaven-workspace` owns backend-neutral paths, views, commands, factories,
  and cleanup. Concrete workspace crates own host/container mechanics.
- `leaven-lm` owns provider-neutral request/response/runtime vocabulary.
  `leaven-lm-cache` wraps neutral LM calls. Provider crates lower concrete APIs.
- `leaven-agent` owns one session over an already-materialized workspace.
  Command/provider crates depend on it, not on core/engine.
- `leaven-agentic` is the adapter layer allowed to know engine/core/stage
  vocabulary; it composes materializers, renderers, runtimes, parsers,
  presenters, and scorers.

Route-away lines worth preserving:

- Vocabulary belongs in neutral contract crates; provider API fields belong in
  provider leaves.
- Store backends may own layout and codecs; `leaven-store` owns capabilities
  only.
- Workspace backends may own host/container mechanics; stage code should use
  `WorkspacePath`, file APIs, and `run_command`.
- LM response caches are not engine evaluation caches; provider prompt caching
  is not Leaven response caching.
- Agent runtimes report sessions; parsers and agentic stages interpret sessions
  into `ProposalBatch` or `Assessment`.
- Codex app-server protocol is leaf-only and topology-tested.

## Review Scout Findings

Current tracked AGENTS files:

- tracked: root `AGENTS.md`, `docs/philosophy/AGENTS.md`, and
  `reviews/2026-05-11-fuckery-extermination-today/AGENTS.md`
- present but untracked at scout time: `docs/AGENTS.md`, `docs/plans/AGENTS.md`,
  `docs/specs/AGENTS.md`, `docs/testing/AGENTS.md`, `crates/AGENTS.md`,
  `examples/AGENTS.md`, `scripts/AGENTS.md`, `xtask/AGENTS.md`

Stale or high-risk guidance:

- Root lists `crates/leaven-dsrs`, but the root `Cargo.toml` has no
  `crates/leaven-dsrs` workspace member and the directory has no manifest.
- Root says to use `jj`, but `jj st` reports this checkout is not currently a
  jj repo; deeper VCS guidance should reflect setup reality.
- `docs/specs/AGENTS.md` elevates the topology spec, but that spec has stale
  topology artifacts.
- `examples/AGENTS.md` says use `just milestone-examples`; the root `Justfile`
  currently makes `milestone-p5` live-Codex by default, so this can spend live
  provider cycles and should not be cited as a cheap/default proof.
- The topology spec names `scripts/check_crate_dag.rs`, but `scripts/` does not
  contain that file.
- `docs/testing/README.md` omits `just milestone-p8` while the root `Justfile`
  defines it.

Boundary proof tools:

- `cargo test -p leaven --test topology_contract` passed in the scout run.
- The topology contract proves the hard-coded workspace member list matches
  root `Cargo.toml`, expected crates have manifests and `src/lib.rs`, examples
  and `xtask` have `src/main.rs`, exact Leaven-to-Leaven workspace dependency
  edges match expectation, `leaven-core` has shallow cold-core leak checks, and
  Codex app-server protocol names stay leaf-only.
- It does not prove every `crates/*` directory is a workspace member; orphan
  `crates/leaven-dsrs` can slip through.
- It does not inspect dev-dependencies, build-dependencies, non-workspace path
  dependencies, feature wiring, public re-export leaks, or actual API shapes.

High-risk bait:

- `crates/leaven-dsrs/src/artifact.rs`: orphan placeholder code that looks like
  a crate but has no manifest or `src/lib.rs`.
- The topology spec's crate tree: useful as intent, unsafe as current inventory.
- Old plans that route graph/context files into `leaven-core`; current topology
  puts that in `leaven-engine`.
- Raw stage context construction in engine stage contract tests is valid for
  dynamic adapter tests but bad precedent for graph mutation/finalization tests.
- `just milestone-examples` includes live `p5`; do not cite as cheap/default
  proof without calling out provider spend.

## First Integration Shape

Integrate the reports in this order:

1. Treat existing untracked top-level AGENTS files as current workspace state,
   not as disposable drafts. Inspect before editing.
2. Fix or annotate stale high-level map hazards before adding many child files:
   `leaven-dsrs`, jj setup reality, P8 testing docs, and live-provider
   milestone wording.
3. Strengthen `crates/AGENTS.md` only where it improves the family atlas.
4. Add high-value crate child files first:
   `leaven-core`, `leaven-engine`, `leaven-run`, `leaven-kernel`,
   `leaven-surface`, `leaven-evidence`, `leaven-population`, `leaven-gepa`,
   `leaven-eval`, `leaven-artifact-skill`, `leaven-std`, `leaven`,
   `leaven-store`, `leaven-workspace`, `leaven-lm`, `leaven-agent`, and
   `leaven-agentic`.
5. Add provider/backend leaf files where the report found real local hazards:
   `leaven-store-file`, `leaven-workspace-local`, `leaven-lm-cache`,
   `leaven-lm-openai`, `leaven-agent-command`, `leaven-agent-codex*`, and
   `leaven-agentic-skill`.
6. Add deeper/test/example files only when they have a distinct proof model,
   provider/live path, or local bait.
7. Run an adversarial AGENTS review before calling the hierarchy done.

## Review Checklist For Drafted Files

- Does the file answer "where does this concept belong" before listing
  invariants?
- Does every child file add local signal beyond parent guidance?
- Are hard prohibitions tied to plausible wrong moves?
- Are referenced paths and commands current?
- Does verification say what the command proves?
- Does any file turn an old plan or superseded spec into current law without
  reconciling code and tests?
- Does any `lib.rs` guidance accidentally invite behavior into map files?
- Does any provider/runtime/optimizer boundary route facts into a lower crate
  that should refuse them?
