# Map-First AGENTS.md Subagent Reports, Verbatim

Status: raw evidence companion to `hierarchy-checkpoint.md`.
Date: 2026-05-11.

This file preserves the useful subagent reports from the first map-first
AGENTS.md hierarchy wave. The reports are copied verbatim except for the report
section headings added by the coordinator.

## Report: Beads Inspiration Scout

Read-only inspection completed. No files edited.

**Patterns Worth Copying**
- Rich root as constitution plus routing map: [/Users/darin/src/personal/beads-rs/AGENTS.md](/Users/darin/src/personal/beads-rs/AGENTS.md) and [/Users/darin/src/personal/beads-rs/docs/AGENTSMD_INFO.md](/Users/darin/src/personal/beads-rs/docs/AGENTSMD_INFO.md). Works because the root carries worldview, crate ownership, default proof tiers, and hierarchy policy in the only file guaranteed everywhere.
- Child AGENTS as delta, not duplicate: crate roots like [/Users/darin/src/personal/beads-rs/crates/beads-core/AGENTS.md](/Users/darin/src/personal/beads-rs/crates/beads-core/AGENTS.md). Works because each child says what this crate owns, what nearby crates own instead, and which proof loop matches the local claim.
- “NEVER + route instead” for tempting wrong moves: [/Users/darin/src/personal/beads-rs/crates/beads-daemon/AGENTS.md](/Users/darin/src/personal/beads-rs/crates/beads-daemon/AGENTS.md), [/Users/darin/src/personal/beads-rs/crates/beads-rs/AGENTS.md](/Users/darin/src/personal/beads-rs/crates/beads-rs/AGENTS.md). Works because prohibitions are tied to observed architectural bait, not generic purity.
- Canonical extension paths in seam directories: [/Users/darin/src/personal/beads-rs/crates/beads-daemon/src/runtime/AGENTS.md](/Users/darin/src/personal/beads-rs/crates/beads-daemon/src/runtime/AGENTS.md), [/Users/darin/src/personal/beads-rs/crates/beads-surface/src/ipc/AGENTS.md](/Users/darin/src/personal/beads-rs/crates/beads-surface/src/ipc/AGENTS.md). Works because it tells an agent the exact module path a change should flow through.
- Test-root proof models: [/Users/darin/src/personal/beads-rs/crates/beads-core/tests/AGENTS.md](/Users/darin/src/personal/beads-rs/crates/beads-core/tests/AGENTS.md), [/Users/darin/src/personal/beads-rs/crates/beads-daemon/tests/AGENTS.md](/Users/darin/src/personal/beads-rs/crates/beads-daemon/tests/AGENTS.md), [/Users/darin/src/personal/beads-rs/crates/beads-rs/tests/integration/fixtures/AGENTS.md](/Users/darin/src/personal/beads-rs/crates/beads-rs/tests/integration/fixtures/AGENTS.md). Works because tests are described by what they prove, fixture stack, and when to move coverage lower.
- Docs authority ladder: [/Users/darin/src/personal/beads-rs/docs/AGENTS.md](/Users/darin/src/personal/beads-rs/docs/AGENTS.md), [/Users/darin/src/personal/beads-rs/docs/architecture/AGENTS.md](/Users/darin/src/personal/beads-rs/docs/architecture/AGENTS.md), [/Users/darin/src/personal/beads-rs/docs/plans/AGENTS.md](/Users/darin/src/personal/beads-rs/docs/plans/AGENTS.md). Works because dated evidence, durable specs, philosophy, and plans are not treated as equal authority.
- Tooling side-effect taxonomy: [/Users/darin/src/personal/beads-rs/scripts/AGENTS.md](/Users/darin/src/personal/beads-rs/scripts/AGENTS.md). Works because scripts are classified by risk and proof method, not just listed.

**Patterns NOT To Copy**
- Beads-specific `bd prime`, bead IDs in every commit, and many-commit-per-bead rhythm do not fit Leaven unless Leaven formally adopts beads as its tracker.
- Git-backed CRDT/store-ref mental model is product-specific. Leaven should keep its own spec-first optimizer/library topology instead.
- Beads’ package assembly crate history maps to Leaven only conceptually. Do not copy names like “assembly/product seam” blindly; use Leaven’s `leaven`, `leaven-run`, examples, and provider-adapter language.
- Tailnet/proxy/daemon slow-suite rules are too specific. Leaven equivalents should be provider/workspace/agent integration slow lanes only where those exist.
- Beads’ legacy/quarantine wording is useful only where Leaven has actual stale or superseded bait. Do not manufacture “legacy bait” sections without evidence.

**Suggested Leaven Equivalents**
- Root [/Users/darin/src/personal/leaven/AGENTS.md](/Users/darin/src/personal/leaven/AGENTS.md): keep as rich constitution, but add only hierarchy policy that is Leaven-specific and not already duplicated from [/Users/darin/src/personal/leaven/docs/AGENTSMD_INFO.md](/Users/darin/src/personal/leaven/docs/AGENTSMD_INFO.md).
- [/Users/darin/src/personal/leaven/crates/AGENTS.md](/Users/darin/src/personal/leaven/crates/AGENTS.md): keep family map, then add child AGENTS at first-class crate roots once a crate has real local proof model or bait: likely `leaven-core`, `leaven-engine`, `leaven-run`, `leaven-lm*`, `leaven-agent*`, `leaven-workspace*`, `leaven-store*`, optimizer crates.
- `crates/leaven-core/AGENTS.md`: beads-core style boundary card for cold algebra, proposal/evaluation/evidence/preference vocabulary, and contract tests.
- `crates/leaven-engine/AGENTS.md`: beads-daemon runtime style extension-path file for `RunGraph`, `RunContext`, stage traits, budget ledger, events, and cache. This is probably Leaven’s most important missing child.
- `crates/leaven-run/AGENTS.md` and `crates/leaven/AGENTS.md`: beads-rs assembly-seam pattern, but worded as product-builder ergonomics and umbrella re-export boundaries.
- Provider families: `crates/leaven-lm*/AGENTS.md`, `crates/leaven-agent*/AGENTS.md`, `crates/leaven-workspace*/AGENTS.md`: seam-directory pattern for neutral traits versus provider lowering and protocol ownership.
- Test roots under high-risk crates: `crates/leaven-engine/tests/AGENTS.md`, `crates/leaven-run/tests/AGENTS.md`, provider tests, and example/milestone tests. Copy the proof-model style, not the beads fixture names.
- Docs homes already align well: [/Users/darin/src/personal/leaven/docs/AGENTS.md](/Users/darin/src/personal/leaven/docs/AGENTS.md), `docs/specs`, `docs/plans`, `docs/testing`, `docs/philosophy`. The beads pattern says to keep sharpening authority/status lines there.
- [/Users/darin/src/personal/leaven/scripts/AGENTS.md](/Users/darin/src/personal/leaven/scripts/AGENTS.md) and [/Users/darin/src/personal/leaven/xtask/AGENTS.md](/Users/darin/src/personal/leaven/xtask/AGENTS.md): already have the right shape; copy beads’ risk-family proof language if scripts diversify.

**Wording Examples**
- `Boundary: this crate owns cold proposal/evaluation vocabulary only; graph, runtime, provider, and store facts must stop at the crate edge.`
- `Routing: new RunGraph mutation affordances start in RunContext; tests should not expose graph internals to make assertions easier.`
- `NEVER: add a provider-specific request shape to leaven-agent or leaven-lm just because one adapter needs it; lower it in the provider crate.`
- `Canonical path: public builder ergonomic -> leaven-run facade -> engine capability -> owning lower crate, with no optimizer strategy state in leaven-run.`
- `Tests: keep algebra laws in the owning cold crate; use examples only when the claim is a public milestone workflow.`
- `Docs: dated plans can explain why a boundary changed, but specs and topology tests own the current rule.`
- `Verification: run the narrow crate test for the changed contract first; use just check only as the completion gate.`
- `Bait: lib.rs is a map, not a convenience drawer; add behavior to the named owning module and re-export deliberately.`
- `Provider seam: neutral request/response vocabulary lives in leaven-lm; HTTP lowering, retries, and API quirks live in leaven-lm-*.`

## Report: Substrate / Execution Scout

Read-only pass complete. I did not edit files or run tests.

**Area Summary**
- Substrate splits cleanly into mechanical primitives (`leaven-kernel`), cold optimizer algebra (`leaven-core`), and explicit artifact projections (`leaven-surface`).
- Execution lives in `leaven-engine`: `RunGraph` storage, `RunContext` mutation, budgets, cache, trust/read scopes, stage traits, events, persistence, and reports.
- Public ergonomics live in `leaven-run`; `leaven` is import experience plus end-to-end/topology tests, not an implementation bucket.
- Existing [crates/AGENTS.md](/Users/darin/src/personal/leaven/crates/AGENTS.md) is directionally correct but too high-level for common wrong placements inside these crates.
- There are no child AGENTS files under the audited crates today.

**Concept Routes**
- Mechanical IDs/cost/errors/fingerprints/metadata/time -> `crates/leaven-kernel/src/{ids,cost,error,fingerprint,metadata,time}.rs` -> [ids.rs](/Users/darin/src/personal/leaven/crates/leaven-kernel/src/ids.rs:1), [cost.rs](/Users/darin/src/personal/leaven/crates/leaven-kernel/src/cost.rs:1), [identity_metadata.rs](/Users/darin/src/personal/leaven/crates/leaven-kernel/tests/identity_metadata.rs:11)
- Artifact identity/apply/cache identity -> `crates/leaven-core/src/artifact.rs` -> [artifact.rs](/Users/darin/src/personal/leaven/crates/leaven-core/src/artifact.rs:5)
- Proposal effect/provenance/batches -> `crates/leaven-core/src/proposal.rs` -> [proposal.rs](/Users/darin/src/personal/leaven/crates/leaven-core/src/proposal.rs:20), [proposal_contract.rs](/Users/darin/src/personal/leaven/crates/leaven-core/tests/proposal_contract.rs:21)
- Evaluation shape/resolution vocabulary -> `crates/leaven-core/src/evaluation.rs` -> [evaluation.rs](/Users/darin/src/personal/leaven/crates/leaven-core/src/evaluation.rs:29)
- Artifact parts/projections -> `crates/leaven-surface/src/*` -> [edit_surface.rs](/Users/darin/src/personal/leaven/crates/leaven-surface/src/edit_surface.rs:7), [part_contract.rs](/Users/darin/src/personal/leaven/crates/leaven-surface/tests/part_contract.rs:9)
- Run graph truth/mutation -> `crates/leaven-engine/src/graph/*` + `context/run_context.rs` -> [storage.rs](/Users/darin/src/personal/leaven/crates/leaven-engine/src/graph/storage.rs:132), [run_context.rs](/Users/darin/src/personal/leaven/crates/leaven-engine/src/context/run_context.rs:27)
- Stage contracts -> `crates/leaven-engine/src/stage/*` -> [optimizer.rs](/Users/darin/src/personal/leaven/crates/leaven-engine/src/stage/optimizer.rs:9), [proposer.rs](/Users/darin/src/personal/leaven/crates/leaven-engine/src/stage/proposer.rs:22), [evaluator.rs](/Users/darin/src/personal/leaven/crates/leaven-engine/src/stage/evaluator.rs:11), [renderer.rs](/Users/darin/src/personal/leaven/crates/leaven-engine/src/stage/renderer.rs:8)
- Product builder lowering -> `crates/leaven-run/src/*` -> [builder.rs](/Users/darin/src/personal/leaven/crates/leaven-run/src/builder.rs:54), [optimize_builder.rs](/Users/darin/src/personal/leaven/crates/leaven-run/tests/optimize_builder.rs:23)
- Umbrella import/e2e contracts -> `crates/leaven/src/*`, `crates/leaven/tests/*` -> [lib.rs](/Users/darin/src/personal/leaven/crates/leaven/src/lib.rs:1), [scalar_keep_best.rs](/Users/darin/src/personal/leaven/crates/leaven/tests/scalar_keep_best.rs:17), [topology_contract.rs](/Users/darin/src/personal/leaven/crates/leaven/tests/topology_contract.rs:421)

**Candidate AGENTS Homes**
- `/Users/darin/src/personal/leaven/crates/leaven-kernel/AGENTS.md`: substrate mechanics boundary; useful to stop optimizer/algebra vocabulary from leaking into IDs, costs, errors, metadata, and time.
- `/Users/darin/src/personal/leaven/crates/leaven-core/AGENTS.md`: cold algebra boundary; useful because artifact/proposal/evaluation are tempting places to add surfaces, graph helpers, stores, or engine convenience.
- `/Users/darin/src/personal/leaven/crates/leaven-surface/AGENTS.md`: projection seam; useful to encode “surfaces are chosen lenses, not artifact truth,” rename identity rules, and fingerprint/cache hazards.
- `/Users/darin/src/personal/leaven/crates/leaven-engine/AGENTS.md`: execution boundary; useful because `RunContext`/`RunGraph`/stage traits/cache/trust/persistence are dense and easy to route into incorrectly.
- `/Users/darin/src/personal/leaven/crates/leaven-run/AGENTS.md`: product-builder lowering; useful to keep public ergonomics, train/validation/test policy, default store/evaluator wiring, and trust hiding out of engine.
- `/Users/darin/src/personal/leaven/crates/leaven/AGENTS.md`: umbrella/import and cross-crate contract tests; useful to keep re-exports and e2e tests distinct from implementation.

**Route-Away Guidance**
- `leaven-kernel`: send artifacts, proposals, evaluation semantics, graph, workspace, store, renderer, and optimizer policy elsewhere.
- `leaven-core`: send parts/projections to `leaven-surface`; graph mutation, cache, budgets, trust, stage traits to `leaven-engine`; reusable evidence/preference/population implementations to their standard crates.
- `leaven-surface`: send artifact-native state and apply laws to `leaven-core`; GEPA-specific part selection/lowering policy to `leaven-gepa`; workspace materialization/rendering to `leaven-render`/agentic crates.
- `leaven-engine`: send optimizer strategy state to optimizer crates; product defaults to `leaven-run`; reusable populations/preferences/evidence to standard crates; concrete providers/backends to adapter crates.
- `leaven-run`: send graph internals and stage traits down to `leaven-engine`; reusable evidence/dataset/report primitives to `leaven-evidence`/`leaven-eval`; import-only shaping to `leaven`.
- `leaven`: send all behavior down; keep only curated re-exports, feature gates, preludes, and cross-crate contract/e2e tests.

**Spec/Code Mismatches Or Stale Docs**
- [leaven_v0_2_1b_corrected_crate_topology_lib_rs.md](/Users/darin/src/personal/leaven/docs/specs/leaven_v0_2_1b_corrected_crate_topology_lib_rs.md:5) is marked v0.2.3/pre-implementation, while [initial_library.md](/Users/darin/src/personal/leaven/docs/specs/initial_library.md:5) is v0.2.7 and actual topology includes later crates/examples.
- The topology spec workspace layout omits `leaven-artifact-skill` and examples `p5`-`p8`; [topology_contract.rs](/Users/darin/src/personal/leaven/crates/leaven/tests/topology_contract.rs:5) includes them.
- The topology spec engine dependency list omits `leaven-surface` at [lines 287-293](/Users/darin/src/personal/leaven/docs/specs/leaven_v0_2_1b_corrected_crate_topology_lib_rs.md:287); actual topology contract includes it at [lines 260-267](/Users/darin/src/personal/leaven/crates/leaven/tests/topology_contract.rs:260). I did not find direct `leaven_surface` source usage in engine, so this is either reserved dependency drift or removable dependency drift.
- The topology spec `leaven-run` dependency list omits `leaven-evidence`; actual builder uses it and topology contract includes it at [lines 339-348](/Users/darin/src/personal/leaven/crates/leaven/tests/topology_contract.rs:339).
- [docs/testing/README.md](/Users/darin/src/personal/leaven/docs/testing/README.md:96) current suites omit audited live suites `crates/leaven-engine/tests/evaluator_registry.rs`, `crates/leaven-run/tests/optimize_builder.rs`, and `crates/leaven-run/tests/scoring_evaluator.rs`.

**First-Draft Map Wording**
- `leaven-kernel`: Own only universal mechanical primitives. Add IDs/cost/error/fingerprint/metadata/time here only when the concept can be used without importing artifact, proposal, evaluation, graph, store, or workspace vocabulary.
- `leaven-core`: Own cold algebra only: artifact identity/apply/cache identity, proposal effect/provenance, evaluation request/set/assessment vocabulary, evidence marker, preference result, and problem associated types. Do not add graph, surface, workspace, store, renderer, population, or optimizer behavior here.
- `leaven-surface`: Own chosen projections over artifacts. A surface may expose parts, addresses, views, and surface edits, but it must lower to artifact-native changes and must not claim decomposition is intrinsic to the artifact.
- `leaven-engine`: Own execution truth: `RunGraph`, `RunContext`, stage traits, budget ledger, cache, trust/read scopes, events, reports, and persistence. Graph mutation stays private behind `RunContext`.
- `leaven-run`: Own the ordinary user builder and lowering into engine/eval/store. Put train/validation/test ergonomics, default evaluator/store wiring, and public result facades here, not in engine or umbrella.
- `leaven`: Own import shape only. Add re-exports, feature-gated modules, preludes, and cross-crate contract tests here; route implementation into the owning crate.

## Report: Algorithms / Vocabulary / Examples Scout

**Area Summary**
- Existing root plus [crates/AGENTS.md](/Users/darin/src/personal/leaven/crates/AGENTS.md:6) and [examples/AGENTS.md](/Users/darin/src/personal/leaven/examples/AGENTS.md:6) already give the high-level map; this area needs local route cards, not another broad taxonomy.
- Strong immediate child homes: `leaven-evidence`, `leaven-population`, `leaven-gepa`, `leaven-eval`, and `leaven-artifact-skill`.
- `leaven-artifacts`, `leaven-render`, `leaven-mipro`, `leaven-textgrad`, and `leaven-trace` are still mostly skeleton/map surfaces, so local AGENTS files there should wait unless implementation starts.
- `leaven-std` deserves a tiny route-away file because it is an attractive wrong implementation bucket.
- Examples P0-P4 are covered by the existing examples map; P5 and P8 are complex enough to justify local files.

**Concept Routes**
- artifact: `leaven-core` owns the trait vocabulary; `crates/leaven-artifacts` owns generic reusable artifacts; `crates/leaven-artifact-*` owns concrete artifact families and their surfaces.
- evidence/eval: [leaven-evidence](/Users/darin/src/personal/leaven/crates/leaven-evidence/src/lib.rs:66) owns reusable evidence shapes; [leaven-eval](/Users/darin/src/personal/leaven/crates/leaven-eval/src/lib.rs:1) owns dataset/split/report vocabulary, not execution.
- preference/population: `leaven-preference` owns stateless relations; [leaven-population](/Users/darin/src/personal/leaven/crates/leaven-population/src/lib.rs:14) owns archive/frontier/fitted state such as `KeepBest`, `ParetoFrontier`, and `TournamentPopulation`.
- render: [leaven-render](/Users/darin/src/personal/leaven/crates/leaven-render/src/lib.rs:1) owns reusable renderer/materializer names; engine owns stage traits/context; workspace backends own filesystem execution.
- optimizer: [leaven-gepa](/Users/darin/src/personal/leaven/crates/leaven-gepa/src/lib.rs:1), `leaven-mipro`, `leaven-textgrad`, and `leaven-trace` own algorithm rhythm/state, not engine machinery.
- examples: [examples/AGENTS.md](/Users/darin/src/personal/leaven/examples/AGENTS.md:17) says examples are runnable acceptance tests and may only keep tiny local fixtures.

**Candidate AGENTS Homes**
- `/Users/darin/src/personal/leaven/crates/leaven-evidence/AGENTS.md`: evidence shape rules, attribution/casewise/pairwise/command routes, and proof anchors.
- `/Users/darin/src/personal/leaven/crates/leaven-population/AGENTS.md`: stateful populations/frontiers/fitted models, event emission, and no graph mutation.
- `/Users/darin/src/personal/leaven/crates/leaven-gepa/AGENTS.md`: GEPA strategy slots, surface lowering, checkpoint/private state, no provider/backend/domain leakage.
- `/Users/darin/src/personal/leaven/crates/leaven-eval/AGENTS.md`: train/validation/test split vocabulary and reports; route execution to engine/run.
- `/Users/darin/src/personal/leaven/crates/leaven-artifact-skill/AGENTS.md`: skill bank validation, skill surfaces, and route-away from agent runtime/paper prompts.
- `/Users/darin/src/personal/leaven/crates/leaven-std/AGENTS.md`: facade-only warning; re-export curation, no behavior.
- Defer: `/Users/darin/src/personal/leaven/crates/leaven-mipro/AGENTS.md`, `leaven-textgrad/AGENTS.md`, `leaven-trace/AGENTS.md` until behavior lands beyond skeleton names.
- Likely useful: `/Users/darin/src/personal/leaven/examples/p5_evoskill_iteration/AGENTS.md` and `/Users/darin/src/personal/leaven/examples/p8_aime_gepa/AGENTS.md` because they have distinct live/provider and public-product proof paths.

**Route-Away Guidance**
- Do not put implementation in `leaven-std`; [it re-exports](/Users/darin/src/personal/leaven/crates/leaven-std/src/lib.rs:1).
- Do not put fitted Bradley-Terry or tournament state in `leaven-preference`; route to `leaven-population`.
- Do not put GEPA selectors, gates, reflection policy, or surface lowering in `leaven-engine`; route to `leaven-gepa`.
- Do not put reusable evidence/population/materializer behavior in `examples/p*`; examples only prove public workflows.
- Do not put provider CLI/runtime details in optimizer crates; route to `leaven-agent*`, `leaven-lm*`, or workspace/provider adapters.

**Canonical Anchors**
- Crate family map: [crates/AGENTS.md](/Users/darin/src/personal/leaven/crates/AGENTS.md:6)
- Example contract: [examples/AGENTS.md](/Users/darin/src/personal/leaven/examples/AGENTS.md:1)
- Testing proof model: [docs/testing/README.md](/Users/darin/src/personal/leaven/docs/testing/README.md:70)
- GEPA reusable loop: [optimizer.rs](/Users/darin/src/personal/leaven/crates/leaven-gepa/src/optimizer.rs:30)
- Skill surfaces: [surface.rs](/Users/darin/src/personal/leaven/crates/leaven-artifact-skill/src/surface.rs:1)
- Public GEPA example: [p8 main](/Users/darin/src/personal/leaven/examples/p8_aime_gepa/src/main.rs:75)
- P5 materializer/renderer split: [p5 main](/Users/darin/src/personal/leaven/examples/p5_evoskill_iteration/src/main.rs:1404)

**First-Draft Map Wording**
- Reusable vocabulary crates own public shapes and laws, not optimizer rhythm or example-specific fixtures.
- Evidence is measurement/attribution payload. Preference interprets evidence. Population owns stateful archive/frontier/model updates.
- Optimizer crates own search rhythm and strategy slots. Engine owns graph/context/runtime services only.
- `leaven-std` is a curated import surface; move behavior to the crate that owns the concept, then re-export it if appropriate.
- Examples are executable acceptance tests. If two examples need the same helper, promote it to the owning library crate.

No files were edited. I did not run test gates; this was a read-only discovery pass. `jj st` also reports this checkout is not currently a `jj` repo.

## Report: Spec Atlas Scout

No files edited.

**Canonical Vs Historical Docs**
- Active governing product spec: [docs/specs/initial_library.md](/Users/darin/src/personal/leaven/docs/specs/initial_library.md:1). Current architecture truth: engine runs optimizer, optimizer owns rhythm, `RunContext` is shared service path, `RunGraph` records truth.
- Active requirement filter: [docs/specs/guiding_principles.md](/Users/darin/src/personal/leaven/docs/specs/guiding_principles.md:1). Requirements/taste only; not crate layout.
- Active topology contract: [docs/specs/leaven_v0_2_1b_corrected_crate_topology_lib_rs.md](/Users/darin/src/personal/leaven/docs/specs/leaven_v0_2_1b_corrected_crate_topology_lib_rs.md:1). Use for crate ownership, forbidden edges, `lib.rs` map discipline.
- Active proof contract: [docs/testing/README.md](/Users/darin/src/personal/leaven/docs/testing/README.md:1). Canonical verification, SLA, coverage ratchet, suite ownership.
- AGENTS rollout rubric: [docs/AGENTSMD_INFO.md](/Users/darin/src/personal/leaven/docs/AGENTSMD_INFO.md:1). Decision-changing hierarchy rules; not product architecture.
- Companion specs: `agentic_stage_runtime.md`, `agentic_skill_optimization_primitives.md`, `agentic_task_execution_substrate.md`, `agentic_library_user_journey.md`, `codex_*_agent_runtime.md`, `lm_runtime_and_response_cache.md`, and milestone contracts. They own narrower surfaces only.
- Planning-only: [docs/plans](/Users/darin/src/personal/leaven/docs/plans/AGENTS.md:1), `eval_lowering_detail.md`, `eval_nomenclature.md`, `gepa_optimizer_surface.md`, `gepa_public_private_surface.md`. Useful, but must be reconciled with current specs/code/tests.
- Superseded/historical: `first_two_subsystems.md`. Do not implement from it without updating to current topology.
- Decision-filter docs: `docs/philosophy/*`. Convert into concrete specs, tests, crate docs, or AGENTS rules before treating as operational.

**Concept Atlas**
| Concept | Owning crate/family | Canonical anchor |
|---|---|---|
| `Amount`, `Cost`, IDs, metadata, fingerprints, durable errors | `leaven-kernel` | topology §4; initial §§6, 24 |
| `Artifact`, `Change`, `ArtifactIdentity`, `CacheIdentity` | `leaven-core` | initial §5.1; topology §5 |
| `EditSurface`, parts, addresses, surface fingerprints | `leaven-surface`; artifact-specific crates for concrete surfaces | initial §§5.1, 24; topology §6 |
| `Candidate`, `RunGraph`, graph views, lineage | `leaven-engine`; IDs in `leaven-kernel` | initial §§5.2-5.3, 10; topology §10 |
| `Proposal`, `ProposalEffect`, `CausalInputs`, `InfoRef` | `leaven-core`; graph validation in `leaven-engine` | initial §5.5, §24 |
| `EvaluationSet`, `EvaluationRequest`, `Assessment` | cold shape in `leaven-core`; resolution/execution in `leaven-engine`; lowered product data in `leaven-eval` | initial §§5.8-5.13; topology §§5, 7.1, 10 |
| Evidence marker and capabilities | marker in `leaven-core`; `CasewiseEvidence`, `AttributableEvidence`, scalar/pairwise/listwise shapes in `leaven-evidence` | initial §5.14; topology §7 |
| Preference | `Preference` result in `leaven-core`; relation trait in `leaven-engine`; stateless impls in `leaven-preference`; fitted models in `leaven-population` | initial §§5.15, 14-15; topology §§16-17 |
| Population/frontier/niche | trait in `leaven-engine`; standard impls in `leaven-population`; GEPA selectors in `leaven-gepa` | initial §§5.16-5.17, 15, 20 |
| Engine, optimizer, `RunContext`, events, trust, cache | `leaven-engine` | initial §§7-8, 16-18; topology §10 |
| Renderer/materializer | traits in `leaven-engine`; reusable impls in `leaven-render`; workspace substrate in `leaven-workspace` | initial §§5.18, 13, 16.6; topology §§10, 15 |
| Workspace and `WorkspacePath` | `leaven-workspace`; backend crates in `leaven-workspace-*` | initial §16.6; topology §9 |
| Storage | `leaven-store`; concrete backends in `leaven-store-*`; graph persistence codec stays engine-owned | initial §19; topology §8 |
| LM runtime/cache/providers | `leaven-lm`, `leaven-lm-cache`, `leaven-lm-*` | topology §18; `lm_runtime_and_response_cache.md` |
| Agent runtime and agentic stages | `leaven-agent` for provider-neutral sessions; `leaven-agent-*` for providers; `leaven-agentic*` for Leaven stage adapters | initial §§13.5, 16; topology §§19-20 |
| Optimizer algorithms | `leaven-gepa`, `leaven-mipro`, `leaven-textgrad`, `leaven-trace` | initial §§20, 22; topology §§21-22 |
| Product builder and import experience | `leaven-run`; umbrella `leaven` only re-exports | initial §22; topology §§7.2, 25 |
| Milestone executable behavior | `examples/pN_*` packages | `milestone_examples_behavioral_contract.md`; docs/testing README |

**First AGENTS.md Homes**
- `docs/specs`: should exist. It owns authority/status discipline for durable contracts and prevents superseded/planning specs from being copied as current law.
- `docs/plans`: should exist. It keeps dated execution notes from masquerading as source of truth.
- `docs/testing`: should exist. The proof model is local and materially different from product/spec docs.
- `examples`: should exist. Milestone packages are executable acceptance tests, not snippets.
- `scripts`: should exist. Python scripts have local side effects and canonical-gate implications.
- `xtask`: should exist separately from `scripts`; Rust automation has different dependency and verification rules.
- `crates`: should exist. The workspace is flat, so one crate-family map is needed before any per-crate file.
- Per-crate child files: do not spray them by default. First likely homes, if area workers confirm local deltas, are `crates/leaven-core`, `crates/leaven-engine`, `crates/leaven-workspace`, `crates/leaven-agent`, `crates/leaven-agentic`, `crates/leaven-gepa`, and `crates/leaven-run`. Each has a distinct proof model or boundary hazard. Thin map/facade crates should inherit from `crates/AGENTS.md` until they gain local behavior.
- `reviews/.../AGENTS.md`: not a first rollout home; treat as local review/audit context only.

**Root/Crates Map Themes**
- Spec-first placement: read `initial_library.md`, `guiding_principles.md`, topology, and the narrow companion spec before coding.
- Topology discipline: identify which crate is allowed to know each fact before choosing a module.
- Cold core stays cold: no graph, runtime, workspace, store backend, surface, provider, GEPA, or adapter leakage.
- `RunContext` is the mutation path; `RunGraph` is durable truth, not strategy opinion.
- Surfaces are selected projections, not intrinsic artifact components.
- Evidence is not preference; casewise measurement and attribution are separate.
- Renderer returns values; materializer writes workspaces. No `WorkspaceRenderer` revival.
- `WorkspacePath` is backend-neutral; host paths are not public workspace API.
- Agent runtimes execute sessions over workspaces; agentic crates adapt outputs into Leaven proposals/assessments.
- `lib.rs` files are maps only: modules, curated re-exports, preludes.
- Tests must name claims and use the lowest clean layer; no public test holes.
- Verification is layered: root owns `just check`; children add deltas, not repeated command spam.

**Open Risks**
- The topology spec is still marked pre-implementation and contains stale manifest details versus current `Cargo.toml` (`resolver = "3"`, newer dependency versions, P8, `leaven-artifact-skill`). Do not copy manifest snippets blindly.
- `docs/specs/tracing-vision/README.md` names `first_two_subsystems.md` as a governing spec even though `first_two_subsystems.md` is explicitly superseded. Reconcile before tracing work.
- `docs/testing/README.md` command list stops at `just milestone-p7`, while `Justfile` and `examples/AGENTS.md` include `milestone-p8`.
- `docs/specs/philosophy_compliance_cleanup.md` lacks a clear status line; classify it before treating it as a spec.
- Several 2026-05-10 specs are “planning” specs. Area workers must verify current code/tests before promoting their vocabulary into AGENTS text.
- No per-crate AGENTS files currently exist. The first crate-level rollout should inspect real code/test hazards before adding children, especially for engine, agentic, GEPA, eval/run, and provider crates.

## Report: Review Scout

No files edited. I treated this as current-filesystem inventory, but note: Git currently tracks only [AGENTS.md](/Users/darin/src/personal/leaven/AGENTS.md:1), [docs/philosophy/AGENTS.md](/Users/darin/src/personal/leaven/docs/philosophy/AGENTS.md:1), and [reviews/2026-05-11-fuckery-extermination-today/AGENTS.md](/Users/darin/src/personal/leaven/reviews/2026-05-11-fuckery-extermination-today/AGENTS.md:1). The other child `AGENTS.md` files are present but untracked.

**Existing AGENTS Inventory**
- Valid: [docs/philosophy/AGENTS.md](/Users/darin/src/personal/leaven/docs/philosophy/AGENTS.md:1), [docs/AGENTS.md](/Users/darin/src/personal/leaven/docs/AGENTS.md:1), [docs/plans/AGENTS.md](/Users/darin/src/personal/leaven/docs/plans/AGENTS.md:1), [docs/testing/AGENTS.md](/Users/darin/src/personal/leaven/docs/testing/AGENTS.md:1), [scripts/AGENTS.md](/Users/darin/src/personal/leaven/scripts/AGENTS.md:1), [xtask/AGENTS.md](/Users/darin/src/personal/leaven/xtask/AGENTS.md:1), [reviews/2026-05-11-fuckery-extermination-today/AGENTS.md](/Users/darin/src/personal/leaven/reviews/2026-05-11-fuckery-extermination-today/AGENTS.md:1).
- Valid but high-risk: [crates/AGENTS.md](/Users/darin/src/personal/leaven/crates/AGENTS.md:1) is directionally right but too broad for the most dangerous crate families; engine/run/agentic/lm/gepa need local deltas.
- Stale/high-risk: root [AGENTS.md](/Users/darin/src/personal/leaven/AGENTS.md:25) names `crates/leaven-dsrs` as a domain adapter, but root [Cargo.toml](/Users/darin/src/personal/leaven/Cargo.toml:3) has no `crates/leaven-dsrs` workspace member and the directory has no manifest.
- Stale/high-risk: [docs/specs/AGENTS.md](/Users/darin/src/personal/leaven/docs/specs/AGENTS.md:7) elevates the topology spec as a top contract, but that spec still references stale topology artifacts.
- High-risk: [examples/AGENTS.md](/Users/darin/src/personal/leaven/examples/AGENTS.md:23) says use `just milestone-examples`; root [Justfile](/Users/darin/src/personal/leaven/Justfile:37) makes `milestone-p5` live-Codex by default, so `milestone-examples` can spend live provider cycles.
- Missing: local AGENTS files for `crates/leaven-engine`, `crates/leaven-core`, `crates/leaven-run`, `crates/leaven-agent*`, `crates/leaven-agentic*`, `crates/leaven-lm*`, `crates/leaven-gepa`, store/workspace backend families, `examples/p5_evoskill_iteration`, `examples/p8_aime_gepa`, and the orphan/quarantine `crates/leaven-dsrs`.

**Stale Or Contradictory Guidance**
- Root says “Use `jj`” at [AGENTS.md](/Users/darin/src/personal/leaven/AGENTS.md:38), but `jj st` reports this checkout is not a jj repo; only `.git` exists. Drafted guidance should be conditional or include setup reality.
- Root lists `crates/leaven-dsrs` as live topology at [AGENTS.md](/Users/darin/src/personal/leaven/AGENTS.md:25); current workspace members in [Cargo.toml](/Users/darin/src/personal/leaven/Cargo.toml:3) omit it.
- The topology spec says `scripts/check_crate_dag.rs` exists at [docs/specs/leaven_v0_2_1b_corrected_crate_topology_lib_rs.md](/Users/darin/src/personal/leaven/docs/specs/leaven_v0_2_1b_corrected_crate_topology_lib_rs.md:147), but `scripts/` currently has only `lint-line-count.py`, `test-suite-sla.py`, and `coverage-gate.py`.
- The same topology spec still lists `crates/leaven-dsrs` as a workspace member at [docs/specs/leaven_v0_2_1b_corrected_crate_topology_lib_rs.md](/Users/darin/src/personal/leaven/docs/specs/leaven_v0_2_1b_corrected_crate_topology_lib_rs.md:210) and dependency node at [docs/specs/leaven_v0_2_1b_corrected_crate_topology_lib_rs.md](/Users/darin/src/personal/leaven/docs/specs/leaven_v0_2_1b_corrected_crate_topology_lib_rs.md:540).
- [docs/testing/README.md](/Users/darin/src/personal/leaven/docs/testing/README.md:19) omits `just milestone-p8` from the command list, while [Justfile](/Users/darin/src/personal/leaven/Justfile:46) defines it.

**Crate-Boundary Proof Tools**
- I ran `cargo test -p leaven --test topology_contract`; it passed: 4 tests, 0 failures.
- [topology_contract.rs](/Users/darin/src/personal/leaven/crates/leaven/tests/topology_contract.rs:420) proves the hard-coded workspace member list matches root `Cargo.toml`, each expected crate has `Cargo.toml` plus `src/lib.rs`, and examples/xtask have `src/main.rs`.
- [topology_contract.rs](/Users/darin/src/personal/leaven/crates/leaven/tests/topology_contract.rs:445) proves exact Leaven-to-Leaven `[dependencies]` entries that use `workspace = true`.
- [topology_contract.rs](/Users/darin/src/personal/leaven/crates/leaven/tests/topology_contract.rs:461) gives a shallow cold-core leak check: no `pub mod context/graph/stage/engine/workspace/store` in `leaven-core/src/lib.rs`, and no `Decomposable`/`Component` substrings under `leaven-core/src`.
- [topology_contract.rs](/Users/darin/src/personal/leaven/crates/leaven/tests/topology_contract.rs:479) checks Codex app-server protocol names remain leaf-only and absent from umbrella `leaven`.
- It does not prove every `crates/*` directory is a workspace member; [crates/leaven-dsrs/src/artifact.rs](/Users/darin/src/personal/leaven/crates/leaven-dsrs/src/artifact.rs:1) slips through because the test iterates expected crates, not all crate directories.
- It does not inspect `dev-dependencies`, `build-dependencies`, non-`workspace = true` path deps, feature wiring, public re-export leaks, or actual API shapes.

**High-Risk Bait**
- [crates/leaven-dsrs/src/artifact.rs](/Users/darin/src/personal/leaven/crates/leaven-dsrs/src/artifact.rs:1): orphan placeholder code that looks like a crate but has no `Cargo.toml` or `src/lib.rs`.
- [docs/specs/leaven_v0_2_1b_corrected_crate_topology_lib_rs.md](/Users/darin/src/personal/leaven/docs/specs/leaven_v0_2_1b_corrected_crate_topology_lib_rs.md:132): stale topology tree; useful as intent, unsafe as current inventory.
- [docs/plans/2026-05-06-first-two-subsystems-surface.md](/Users/darin/src/personal/leaven/docs/plans/2026-05-06-first-two-subsystems-surface.md:81): old plan routes graph/context files into `leaven-core`; current topology puts that in `leaven-engine`.
- [crates/leaven-engine/tests/stage_trait_contracts.rs](/Users/darin/src/personal/leaven/crates/leaven-engine/tests/stage_trait_contracts.rs:24): raw stage context creation is valid for dynamic adapter contract tests, but bad precedent for graph mutation/finalization tests.
- [Justfile](/Users/darin/src/personal/leaven/Justfile:49): `milestone-examples` includes live `p5`; do not cite it as a cheap/default proof without calling out provider spend.

**Reviewer Checklist**
- Does every AGENTS claim point at current Cargo/workspace reality, not only the topology spec?
- Does each child file add local delta, or is it parent guidance in smaller letters?
- Are stale docs named as stale where agents will see them before copying paths?
- For crate boundaries, does the file say what `topology_contract` proves and what it does not prove?
- Are live/provider/network commands separated from deterministic default gates?
- Are examples described as acceptance scenarios, not implementation buckets?
- Are old plans treated as dated notes unless reconciled with specs and code?
- Are orphan/skeleton/placeholder directories explicitly marked as quarantine or bait?
- Are verification commands tied to change types and proof meaning, not listed naked?

## Report: Interfaces / Adapters Scout

**Area Summary**
- No files edited. Current AGENTS coverage stops at [crates/AGENTS.md](/Users/darin/src/personal/leaven/crates/AGENTS.md:1); these flat crate families do not yet have local child maps.
- The major boundary is already implemented in code: neutral contracts live in `leaven-store`, `leaven-workspace`, `leaven-lm`, and `leaven-agent`; concrete adapters lower into those contracts; `leaven-agentic` converts runtime/session facts into optimizer-stage outputs.
- The strongest candidate child AGENTS homes are the contract crates plus the non-skeleton adapter leaves: local workspace, LM cache/OpenAI, command/Codex runtimes, and agentic/agentic-skill.
- `jj status` reports this checkout is not a jj repo, so I did not use VCS history for placement evidence.

**Family Map**
- Store: [leaven-store](/Users/darin/src/personal/leaven/crates/leaven-store/src/lib.rs:1) owns `BlobStore`, `EvidenceStore`, and `CheckpointStore`; backends such as [leaven-store-inline](/Users/darin/src/personal/leaven/crates/leaven-store-inline/src/store.rs:10) and [leaven-store-file](/Users/darin/src/personal/leaven/crates/leaven-store-file/src/store.rs:11) depend inward on it. Store must not know `RunGraph`.
- Workspace: [leaven-workspace](/Users/darin/src/personal/leaven/crates/leaven-workspace/src/workspace.rs:58) owns backend-neutral paths, views, commands, factories, and cleanup. Concrete workspace crates depend inward; [leaven-workspace-local](/Users/darin/src/personal/leaven/crates/leaven-workspace-local/src/factory.rs:16) is the host-path exception.
- LM: [leaven-lm](/Users/darin/src/personal/leaven/crates/leaven-lm/src/model.rs:9) owns provider-neutral request/response/runtime vocabulary. [leaven-lm-cache](/Users/darin/src/personal/leaven/crates/leaven-lm-cache/src/key.rs:7) wraps neutral LM calls; provider crates such as [leaven-lm-openai](/Users/darin/src/personal/leaven/crates/leaven-lm-openai/src/client.rs:10) lower to concrete APIs.
- Agent: [leaven-agent](/Users/darin/src/personal/leaven/crates/leaven-agent/src/runtime.rs:12) owns one session over an already-materialized workspace. Command and provider crates depend on it, not on core/engine.
- Agentic: [leaven-agentic](/Users/darin/src/personal/leaven/crates/leaven-agentic/src/lib.rs:1) is the adapter layer allowed to know engine/core/stage vocabulary; it composes materializers, renderers, runtimes, parsers, presenters, and scorers.

**Candidate AGENTS Homes**
- `/Users/darin/src/personal/leaven/crates/leaven-store/AGENTS.md`: storage capability boundary; prevent `RunGraph`, backend, and serialization-policy drift into the trait crate.
- `/Users/darin/src/personal/leaven/crates/leaven-store-file/AGENTS.md`: durable local layout, append/resume rules, key validation, `LATEST` checkpoint behavior.
- `/Users/darin/src/personal/leaven/crates/leaven-workspace/AGENTS.md`: backend-neutral workspace laws: `WorkspacePath`, `WorkspaceView`, command execution, cleanup, and no artifact/agent semantics.
- `/Users/darin/src/personal/leaven/crates/leaven-workspace-local/AGENTS.md`: local mount and host-process behavior; command timeout/user/env/stdin semantics are real local hazards.
- `/Users/darin/src/personal/leaven/crates/leaven-lm/AGENTS.md`: neutral LM contract, continuation semantics, request truth, response validation.
- `/Users/darin/src/personal/leaven/crates/leaven-lm-cache/AGENTS.md`: Leaven response cache vs provider prompt cache vs engine evaluation cache.
- `/Users/darin/src/personal/leaven/crates/leaven-lm-openai/AGENTS.md`: OpenAI Responses API lowering leaf; provider IDs/continuation/usage mapping stay here.
- `/Users/darin/src/personal/leaven/crates/leaven-agent/AGENTS.md`: provider-neutral runtime/session/output-contract boundary; no optimizer nouns.
- `/Users/darin/src/personal/leaven/crates/leaven-agent-command/AGENTS.md`: reusable command-backed runtime substrate for CLI providers.
- `/Users/darin/src/personal/leaven/crates/leaven-agent-codex/AGENTS.md`: facade-only rules; no protocol, no runtime logic.
- `/Users/darin/src/personal/leaven/crates/leaven-agent-codex-cli/AGENTS.md`: backend-neutral `codex exec` adapter; native skill layout remains materializer/stage-owned.
- `/Users/darin/src/personal/leaven/crates/leaven-agent-codex-app-server/AGENTS.md`: Codex protocol leaf and stdio/local-mount exception.
- `/Users/darin/src/personal/leaven/crates/leaven-agentic/AGENTS.md`: stage-adapter ownership: graph views may be read, graph mutation still returns typed proposals/assessments.
- `/Users/darin/src/personal/leaven/crates/leaven-agentic-skill/AGENTS.md`: skill-specific materialization/parser rules over `leaven-artifact-skill`.

**Route-Away Guidance**
- Vocabulary belongs in neutral contract crates; provider API fields belong only in provider leaves.
- Store backends may own layout and codecs; `leaven-store` owns capabilities only.
- Workspace backends may own host/container mechanics; stage code should use `WorkspacePath`, file APIs, and `run_command`.
- LM caches are not engine evaluation caches; provider prompt caching is not Leaven response caching.
- Agent runtimes report sessions; parsers and agentic stages interpret sessions into `ProposalBatch` or `Assessment`.
- Codex app-server protocol is leaf-only, enforced by [topology_contract.rs](/Users/darin/src/personal/leaven/crates/leaven/tests/topology_contract.rs:480).

**Canonical Code/Test Anchors**
- Topology: [topology_contract.rs](/Users/darin/src/personal/leaven/crates/leaven/tests/topology_contract.rs:445).
- Workspace laws: [workspace.rs](/Users/darin/src/personal/leaven/crates/leaven-workspace/src/workspace.rs:112), [workspace_view.rs](/Users/darin/src/personal/leaven/crates/leaven-workspace/tests/workspace_view.rs:15), [local_workspace.rs](/Users/darin/src/personal/leaven/crates/leaven-workspace-local/tests/local_workspace.rs:12).
- LM/cache: [model.rs](/Users/darin/src/personal/leaven/crates/leaven-lm/src/model.rs:9), [key.rs](/Users/darin/src/personal/leaven/crates/leaven-lm-cache/src/key.rs:16), [cache_contract.rs](/Users/darin/src/personal/leaven/crates/leaven-lm-cache/tests/cache_contract.rs:77).
- Agent runtime: [runtime.rs](/Users/darin/src/personal/leaven/crates/leaven-agent/src/runtime.rs:12), [session.rs](/Users/darin/src/personal/leaven/crates/leaven-agent/src/session.rs:108), [runtime_contract.rs](/Users/darin/src/personal/leaven/crates/leaven-agent/tests/runtime_contract.rs:399).
- Agentic adapters: [proposer.rs](/Users/darin/src/personal/leaven/crates/leaven-agentic/src/proposer.rs:66), [case_evaluator.rs](/Users/darin/src/personal/leaven/crates/leaven-agentic/src/case_evaluator.rs:47), [agentic_workload.rs](/Users/darin/src/personal/leaven/crates/leaven-agentic/tests/agentic_workload.rs:607).

**First-Draft Map Wording**
- Neutral crates name capabilities and durable vocabulary; concrete crates lower provider/backend mechanics into those capabilities.
- Do not move provider request fields, host paths, graph mutation, optimizer policy, or skill-specific layout into neutral contract crates.
- `AgentRuntime` runs one session in a prepared workspace; agentic stages decide why it ran and parse what it produced.
- `WorkspacePath` is the public path type. Host `PathBuf` is only for backends or explicit local-mount adapters.
- Provider-family facades re-export leaves only; they do not own protocol, parser, or stage behavior.
- For boundary changes, run `cargo test -p leaven --test topology_contract`; for local adapter behavior, run that crate’s focused tests.
