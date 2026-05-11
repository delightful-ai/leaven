# Map-First AGENTS Workspace Coverage

Status: active closeout denominator for the 2026-05-11 hierarchy goal.

This file records coverage decisions for the current Leaven workspace so "no
local child AGENTS.md" is never confused with "forgotten". It is dated
execution evidence; promote stable rules into the nearest owning `AGENTS.md`,
spec, crate doc, or test when they become durable.

## Sources

- Workspace inventory: `cargo metadata --no-deps --format-version 1`.
- Live crate inventory proof: `crates/leaven/tests/topology_contract.rs`.
- Hierarchy standard: `docs/AGENTSMD_INFO.md` and
  `/Users/darin/.agents/skills/hierarchical-agents-md/SKILL.md`.
- Recent audit package:
  `reviews/2026-05-11-fuckery-extermination-today/`.
- Governing handoff:
  `docs/plans/2026-05-11-map-first-agents/goal-handoff.yaml`.
- Raw scouting evidence:
  `docs/plans/2026-05-11-map-first-agents/subagent-reports-verbatim.md`.

## Classification Legend

- `local AGENTS`: this goal added or retained a child `AGENTS.md` because the
  subtree has a real local boundary, hazard, proof model, or routing delta.
- `parent-covered`: parent files are specific enough for current behavior; add
  a child only when local behavior or hazards become distinct.
- `deferred`: retained as a future classification, but no current workspace
  crate remains deferred after the saturation pass. Skeleton/reserved crates
  have local files because their public reservation names are concrete bait.
- `quarantined`: stale, test-only, or bait-like surface that must not be copied
  as precedent without deliberate reintroduction or proof.

## Rust Crates

| Crate | Decision | Local routing source | Reason |
| --- | --- | --- | --- |
| `leaven` | local AGENTS | `crates/leaven/AGENTS.md` | Umbrella import surface, feature gates, and topology/end-to-end tests are a distinct local boundary. |
| `leaven-agent` | local AGENTS | `crates/leaven-agent/AGENTS.md` | Provider-neutral one-session runtime contract must stay separate from agentic stages and provider leaves. |
| `leaven-agent-claude-code` | local AGENTS | `crates/leaven-agent-claude-code/AGENTS.md` | Runtime adapter placeholder has public provider-looking names; local file quarantines scaffold status and routes generic runtime work away. |
| `leaven-agent-codex` | local AGENTS | `crates/leaven-agent-codex/AGENTS.md` | Codex provider-family facade has feature-gated re-export traps. |
| `leaven-agent-codex-app-server` | local AGENTS | `crates/leaven-agent-codex-app-server/AGENTS.md` | Protocol dependencies, stdio local-mount semantics, and vendored Codex drift are leaf-only hazards. |
| `leaven-agent-codex-cli` | local AGENTS | `crates/leaven-agent-codex-cli/AGENTS.md` | Backend-neutral Codex CLI lowering has distinct stable-output and sandbox/approval rules. |
| `leaven-agent-command` | local AGENTS | `crates/leaven-agent-command/AGENTS.md` | Command-backed runtime substrate must remain provider-neutral and workspace-mediated. |
| `leaven-agent-opencode` | local AGENTS | `crates/leaven-agent-opencode/AGENTS.md` | Runtime adapter placeholder has public provider-looking names; local file quarantines scaffold status and routes generic runtime work away. |
| `leaven-agentic` | local AGENTS | `crates/leaven-agentic/AGENTS.md` | Generic agentic stage adapters bridge runtime sessions to proposals/assessments. |
| `leaven-agentic-skill` | local AGENTS | `crates/leaven-agentic-skill/AGENTS.md` | Skill-specific materialization and parsing are distinct from generic agentic and skill artifact ownership. |
| `leaven-artifact-git` | local AGENTS | `crates/leaven-artifact-git/AGENTS.md` | Public Git artifact/surface names are placeholders and need local routing away from workspace lifecycle and skill-bank behavior. |
| `leaven-artifact-jj` | local AGENTS | `crates/leaven-artifact-jj/AGENTS.md` | Public JJ artifact/conflict names are placeholders and need local routing away from workspace command/lifecycle behavior. |
| `leaven-artifact-skill` | local AGENTS | `crates/leaven-artifact-skill/AGENTS.md` | Skill bank is a concrete artifact/surface family, not runtime execution. |
| `leaven-artifacts` | local AGENTS | `crates/leaven-artifacts/AGENTS.md` | Public text/dir/part-map names are reservations and need explicit scaffold quarantine before facade exposure. |
| `leaven-core` | local AGENTS | `crates/leaven-core/AGENTS.md` | Cold optimizer algebra is a high-risk boundary against graph, surface, runtime, and provider leakage. |
| `leaven-cuda` | local AGENTS | `crates/leaven-cuda/AGENTS.md` | CUDA public names imply GPU behavior; local file keeps them scaffolded and routes hardware/live proof separately. |
| `leaven-derive` | local AGENTS | `crates/leaven-derive/AGENTS.md` | Reserved proc-macro derives have a distinct compile-fail contract and `trybuild` proof loop. |
| `leaven-engine` | local AGENTS | `crates/leaven-engine/AGENTS.md` | Run execution, `RunGraph`, and `RunContext` mutation are the central routing hazard. |
| `leaven-eval` | local AGENTS | `crates/leaven-eval/AGENTS.md` | Lowered dataset/split/report vocabulary must not absorb engine execution or run builder ergonomics. |
| `leaven-evidence` | local AGENTS | `crates/leaven-evidence/AGENTS.md` | Reusable evidence values must stay separate from stores, preferences, populations, and evaluators. |
| `leaven-gepa` | local AGENTS | `crates/leaven-gepa/AGENTS.md` | GEPA strategy state, gates, selectors, and surface-edit lowering are optimizer-owned. |
| `leaven-kernel` | local AGENTS | `crates/leaven-kernel/AGENTS.md` | Mechanical substrate must not learn artifact or optimizer semantics. |
| `leaven-lm` | local AGENTS | `crates/leaven-lm/AGENTS.md` | Provider-neutral LM vocabulary is a key adapter boundary. |
| `leaven-lm-anthropic` | local AGENTS | `crates/leaven-lm-anthropic/AGENTS.md` | Provider-looking names are scaffolded and must not copy OpenAI transport/cache semantics. |
| `leaven-lm-cache` | local AGENTS | `crates/leaven-lm-cache/AGENTS.md` | Leaven LM response caching must stay separate from provider transport and engine evaluation cache. |
| `leaven-lm-local` | local AGENTS | `crates/leaven-lm-local/AGENTS.md` | Local-LM provider names are scaffolded and need explicit separation from workspace/process management. |
| `leaven-lm-mock` | quarantined | `crates/leaven-lm-mock/AGENTS.md` | Deterministic scripted test LM; useful locally, but not a live-provider template. |
| `leaven-lm-openai` | local AGENTS | `crates/leaven-lm-openai/AGENTS.md` | OpenAI Responses API lowering has provider-specific continuation/cache traps. |
| `leaven-mipro` | local AGENTS | `crates/leaven-mipro/AGENTS.md` | Optimizer public names are scaffolding and need local warnings against treating them as strategy proof. |
| `leaven-population` | local AGENTS | `crates/leaven-population/AGENTS.md` | Fitted population state must stay separate from stateless preferences and optimizer-specific selectors. |
| `leaven-preference` | local AGENTS | `crates/leaven-preference/AGENTS.md` | Stateless preference relations are easy to confuse with fitted population state. |
| `leaven-python` | local AGENTS | `crates/leaven-python/AGENTS.md` | Python public names imply runtime behavior; local file keeps interpreter/workspace proof boundaries explicit. |
| `leaven-render` | local AGENTS | `crates/leaven-render/AGENTS.md` | Renderer/materializer public names are placeholders and must not serve as GEPA/evidence rendering proof. |
| `leaven-run` | local AGENTS | `crates/leaven-run/AGENTS.md` | Product-builder ergonomics must stay separate from engine internals and optimizer strategy. |
| `leaven-std` | local AGENTS | `crates/leaven-std/AGENTS.md` | Curated facade is map-only with feature-gated import hazards. |
| `leaven-store` | local AGENTS | `crates/leaven-store/AGENTS.md` | Storage capability traits must not absorb graph checkpoint schemas or backend layout. |
| `leaven-store-file` | local AGENTS | `crates/leaven-store-file/AGENTS.md` | Local filesystem backend has concrete layout/reopen/key behavior. |
| `leaven-store-inline` | local AGENTS | `crates/leaven-store-inline/AGENTS.md` | In-memory backend is behavior-bearing and needs local namespace/non-durable/default-backend guidance. |
| `leaven-store-object` | local AGENTS | `crates/leaven-store-object/AGENTS.md` | Object-store public name is scaffolded and needs local auth/retry/key-layout quarantine. |
| `leaven-store-sqlite` | local AGENTS | `crates/leaven-store-sqlite/AGENTS.md` | SQLite public name is scaffolded and needs local schema/migration quarantine. |
| `leaven-surface` | local AGENTS | `crates/leaven-surface/AGENTS.md` | Explicit projections and surface fingerprints are a high-risk cold-core neighbor. |
| `leaven-textgrad` | local AGENTS | `crates/leaven-textgrad/AGENTS.md` | TextGrad public names are scaffolding and need local LM/evidence boundary warnings. |
| `leaven-trace` | local AGENTS | `crates/leaven-trace/AGENTS.md` | Trace optimizer public names are scaffolding and need local separation from engine graph/events. |
| `leaven-workspace` | local AGENTS | `crates/leaven-workspace/AGENTS.md` | Backend-neutral paths, views, command vocabulary, factories, and cleanup are a core substrate boundary. |
| `leaven-workspace-docker` | local AGENTS | `crates/leaven-workspace-docker/AGENTS.md` | Docker factory name is scaffolded and needs host-side-effect/live-test quarantine. |
| `leaven-workspace-e2b` | local AGENTS | `crates/leaven-workspace-e2b/AGENTS.md` | E2B factory name is scaffolded and needs auth/spend/live-sandbox quarantine. |
| `leaven-workspace-firecracker` | local AGENTS | `crates/leaven-workspace-firecracker/AGENTS.md` | Firecracker factory name is scaffolded and needs VM privilege/live-test quarantine. |
| `leaven-workspace-git` | local AGENTS | `crates/leaven-workspace-git/AGENTS.md` | Git workspace factory name is scaffolded and must stay separate from Git artifact identity. |
| `leaven-workspace-k8s` | local AGENTS | `crates/leaven-workspace-k8s/AGENTS.md` | Kubernetes factory name is scaffolded and needs cluster credential/live-test quarantine. |
| `leaven-workspace-local` | local AGENTS | `crates/leaven-workspace-local/AGENTS.md` | Trusted local tempdir/process backend has host-mount and local-command hazards. |

## Workspace Example Packages

| Package | Decision | Local routing source | Reason |
| --- | --- | --- | --- |
| `p0_graph_skeleton` | parent-covered | `examples/AGENTS.md` | Deterministic milestone; parent package map names proof and ownership. |
| `p1_keep_best` | parent-covered | `examples/AGENTS.md` | Deterministic milestone; parent package map names proof and ownership. |
| `p2_pairwise_tournament` | parent-covered | `examples/AGENTS.md` | Deterministic milestone; parent package map names proof and ownership. |
| `p3_gepa_parity` | parent-covered | `examples/AGENTS.md` | Deterministic milestone; parent package map names proof and ownership. |
| `p4_meta_harness_lite` | parent-covered | `examples/AGENTS.md` | Deterministic milestone; parent package map names proof and ownership. |
| `p5_evoskill_iteration` | local AGENTS | `examples/p5_evoskill_iteration/AGENTS.md` | Live Codex/EvoSkill proof path spends provider/runtime resources and needs local warnings. |
| `p6_optimizer_policy_self_opt` | parent-covered | `examples/AGENTS.md` | Deterministic milestone; parent package map names proof and ownership. |
| `p7_self_optimization_kernel` | parent-covered | `examples/AGENTS.md` | Deterministic milestone; parent package map names proof and ownership. |
| `p8_aime_gepa` | local AGENTS | `examples/p8_aime_gepa/AGENTS.md` | Deterministic, cached-data, and live-provider paths must stay separate; current P8 is mechanics/proxy proof, not real reflection or LM/cache product proof. |

## Tooling And Non-Crate Packages

| Path/package | Decision | Local routing source | Reason |
| --- | --- | --- | --- |
| `xtask` | local AGENTS | `xtask/AGENTS.md` | Repo automation package has distinct side-effect and command exposure rules. |
| `scripts` | local AGENTS | `scripts/AGENTS.md` | Script side effects, rerun safety, and Justfile/testing docs alignment are local hazards. |
| `docs` | local AGENTS | `docs/AGENTS.md` | Docs authority ladder is a routing boundary. |
| `docs/specs` | local AGENTS | `docs/specs/AGENTS.md` | Specs are durable contracts, but status lines, P0-P4 milestone spec coverage, stale tracing references, and live Cargo/tests all affect routing. |
| `docs/testing` | local AGENTS | `docs/testing/AGENTS.md` | Proof model, suite SLA, coverage ratchet, milestone-command traps, and product-proof/mechanics/proxy classification are local. |
| `docs/plans` | local AGENTS | `docs/plans/AGENTS.md` | Dated execution notes, raw reports, and handoffs must not outrank specs/code/tests or recent audit findings. |
| `docs/philosophy` | local AGENTS | `docs/philosophy/AGENTS.md` | Philosophy is design pressure and canonical Leaven skill source, not implementation status or audit storage. |
| `reviews/2026-05-11-fuckery-extermination-today` | local AGENTS | `reviews/2026-05-11-fuckery-extermination-today/AGENTS.md` | Latest audit package has local review instructions; root routing marks reviews as evidence/prioritization, not product law. |

## Quarantined Non-Workspace Directory

| Path | Decision | Local routing source | Reason |
| --- | --- | --- | --- |
| `crates/leaven-dsrs` | quarantined | `AGENTS.md`, `crates/AGENTS.md`, `crates/leaven-dsrs/AGENTS.md` | Orphan placeholder directory with no manifest or `src/lib.rs`; route away unless deliberately reintroduced. |

## Verification Evidence

- Worker substrate full gate: `RUSTUP_TOOLCHAIN=nightly just check` passed with
  line coverage `98.56%` and branch coverage `87.97%`.
- Repeated topology checks: `cargo test -p leaven --test topology_contract`
  passed in worker lanes.
- Provider/agent lane: `cargo nextest run -p leaven-lm -p leaven-lm-cache
  -p leaven-lm-openai -p leaven-agent -p leaven-agent-command
  -p leaven-agent-codex-cli -p leaven-agentic -p leaven-agentic-skill`
  passed with 113 tests.
- Known existing failure, now marked in the crate-local AGENTS file:
  `cargo check -p leaven-agent-codex-app-server
  --features app-server` and `--features stdio` currently fail on vendored
  Codex protocol drift around `InitializeCapabilities::request_attestation`.
  The local app-server `AGENTS.md` routes that failure to the app-server leaf
  instead of weakening the topology.
- Example lane sanity: `just --dry-run milestone-examples` confirms it expands
  through `LEAVEN_CODEX_LIVE=1 cargo run -p p5_evoskill_iteration
  -- --live-codex`; therefore it is not a cheap deterministic proof.
- Latest audit-doc pass: `reviews/2026-05-11-fuckery-extermination-today`
  distinguishes topology from public maturity. The hierarchy now classifies P8
  as mechanics/proxy proof and adds warnings for umbrella defaults, run builder
  maturity, GEPA fixed-edit reflection, LM cache role composition, and OpenAI
  model-default semantics.
- Worker E saturation pass: docs/testing/examples/scripts/xtask guidance now
  names the authority ladder, stale spec/plan traps, exact milestone proof
  classifications, coverage-versus-product-proof distinction, coverage P5/P8
  limitations, script side-effect rules, and empty-xtask proof limits.

## Closeout Notes

- This matrix is not completion evidence by itself. It is the denominator the
  adversarial reviewer should test against.
- `just check` and `topology_contract` are hygiene/proof anchors, not a
  substitute for blind routing review.
- After the saturation pass, every workspace crate has a child `AGENTS.md`.
  Placeholder leaves got local files because public reservation names are
  themselves concrete bait; future cleanup should delete a child only when its
  local warning has either become behavior-bearing guidance or been promoted.
