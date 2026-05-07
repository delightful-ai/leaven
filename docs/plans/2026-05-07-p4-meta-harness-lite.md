# P4 Meta-Harness Lite Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Fully satisfy P4 Meta-Harness Lite: side-effectful workspace materialization uses backend-neutral paths, materializers receive the correct actor-scoped graph view, hidden held-out evidence cannot leak into proposer/agent workspaces, a deterministic agentic proposer authors fresh artifacts with `ProposalEffect::Create` and `CausalInputs::None`, evaluation evidence is stored by reference, workspace cleanup is explicit, and the runnable milestone plus focused tests prove the contract.

**Architecture:** Keep workspace path/lifecycle truth in `leaven-workspace`, materializer context and stage traits in `leaven-engine`, reusable command/trajectory evidence in `leaven-evidence`, population updates in `leaven-population`, inline evidence storage in `leaven-store-inline`, and the small Meta-Harness fixture in `examples/p4_meta_harness_lite` plus contract tests. `leaven-core` stays cold and does not learn about workspaces, renderers, materializers, stores, agents, or engine runtime.

**Tech Stack:** Rust 2024, Leaven workspace crates, `leaven-workspace-local` for the runnable local workspace, `leaven-store-inline` for deterministic evidence storage, `futures::executor::block_on` for runnable examples and scenario tests, `proptest` for workspace path laws, focused `cargo nextest` gates, `just milestone-p4`, and final `just check`.

---

## Governing Requirements

- `AGENTS.md`
- `docs/specs/milestone_examples_behavioral_contract.md`, P4 section
- `docs/specs/initial_library.md`, especially renderer/materializer, trust, workspace lifecycle, Prototype 4, and Meta-Harness worked example sections
- `docs/specs/leaven_v0_2_1b_corrected_crate_topology_lib_rs.md`
- `docs/specs/guiding_principles.md`
- `docs/testing/README.md`
- `docs/philosophy/skills/leaven-type-design/SKILL.md`
- `docs/philosophy/skills/leaven-trait-design/SKILL.md`
- `docs/philosophy/skills/leaven-error-design/SKILL.md`
- `docs/philosophy/skills/leaven-test-design/SKILL.md`

## Current Baseline

The live tree already has some P4 substrate:

- `Materializer`, `MaterializationReport`, `MaterializeError`, and `TruncationNote` exist in `leaven-engine`; there are no live `WorkspaceRenderer` public exports or aliases.
- `WorkspacePath`, `Workspace`, `WorkspaceFactory`, `WorkspaceBackend`, and `WorkspaceView` exist in `leaven-workspace`.
- `WorkspacePath` already rejects empty parses, absolute paths, parent traversal, and empty path components.
- `examples/p4_meta_harness_lite` allocates a local workspace, writes a seed harness, creates a fresh proposal, applies it, and awaits cleanup.
- Baseline commands:
  - `just milestone-p4` passes.
  - `cargo nextest run -p leaven-workspace --test workspace_path` passes.
  - `cargo nextest run -p leaven-engine --test materializer_contract` fails because the test target does not exist.

The remaining work is to turn P4 from a thin manual fixture into a real acceptance path: proposer-owned materialization, actor-scoped materializer context, deterministic materializer tests, trajectory evidence, repo-task evaluation, population update, and explicit cleanup proof.

## Non-Negotiables

- No `WorkspaceRenderer` aliases or compatibility paths.
- No host `PathBuf` in public workspace addresses; materializers use `WorkspacePath`.
- No materializer or renderer registry; composition is by explicit stage-owned fields.
- `Workspace::cleanup(self)` remains the authoritative cleanup path.
- `local_mount()` remains optional and the example must not require it.
- Fresh agent-authored artifacts use `ProposalEffect::Create` with `CausalInputs::None`.
- Historical influence is represented by `InfoRef`, not fake causal parents.
- Large evidence stays behind `EvidenceStore`; graph records hold `EvidenceRef`, not trajectory blobs.
- Hidden held-out/test partitions cannot appear in proposer/materializer graph views.
- `lib.rs` files remain maps only.

## Spec-To-Code Map

| Requirement | Owner | Current status | Work |
| --- | --- | --- | --- |
| Public hard cutover to `Materializer` | `leaven-engine`, `leaven-render`, umbrella crate | Implemented; no live aliases | Add a guard test for absence of old names |
| Backend-neutral `WorkspacePath` laws | `leaven-workspace` | Example tests exist | Add generated path law coverage |
| Explicit cleanup | `leaven-workspace` and example | Example awaits cleanup | Add exactly-once cleanup proof |
| Materializer trait and report | `leaven-engine` | Implemented with `RenderContext` | Introduce `MaterializeContext` and route materializers through it |
| Materializers receive actor-scoped graph views | `leaven-engine` | Not proven; proposer-scoped materialization is missing | Add `ProposalContext::materialize_context()` and tests |
| Materializer writes only via `WorkspacePath` | `leaven-workspace`, tests | API shape enforces this | Add deterministic write contract |
| Command/agent trajectory evidence | `leaven-evidence` | Placeholder structs only | Implement minimal reusable records with inline/ref output handles |
| P4 example flow | example crate | Allocates workspace and creates proposal manually | Move materialization into deterministic proposer, run evaluator, store evidence, update population |
| `Create + None + informed_by(history)` | `leaven-core`/`leaven-engine` graph behavior | Create-without-parent covered, informed create not P4-specific | Add P4 contract coverage |

## Task 1: Add MaterializeContext

**Files:**
- Add: `crates/leaven-engine/src/context/materialize_context.rs`
- Modify: `crates/leaven-engine/src/context/mod.rs`
- Modify: `crates/leaven-engine/src/context/run_context.rs`
- Modify: `crates/leaven-engine/src/context/proposal_context.rs`
- Modify: `crates/leaven-engine/src/graph/view.rs`
- Modify: `crates/leaven-engine/src/stage/renderer.rs`
- Modify: `crates/leaven-engine/src/lib.rs`
- Modify: `crates/leaven/src/lib.rs`
- Modify: `crates/leaven/src/prelude.rs`

**Claims:**
- A materializer receives a materialization-specific context, not a value-renderer context.
- A materializer invoked inside a proposer sees the proposer's read scope, so hidden test partitions stay hidden.

**Implementation Shape:**
- Add `MaterializeContext<'a, P>` with `graph`, `budget`, and `read_scope` accessors.
- Change `Materializer::materialize_into` to accept `MaterializeContext<'_, P>`.
- Add `RunContext::materialize_context(stage)` for direct materializer tests and simple users.
- Add `ProposalContext::materialize_context()` that clones the proposer-scoped graph view and read scope, and snapshots the proposer-stage budget.
- Make `RunGraphView` cloneable so context values can reuse the same actor-scoped graph view without public graph mutation.
- Re-export `MaterializeContext` through `leaven-engine` and the umbrella crate.

**Verification:**
- `cargo nextest run -p leaven-engine --test materializer_contract`

## Task 2: Implement Command And Agent Trajectory Evidence

**Files:**
- Modify: `crates/leaven-evidence/src/lib.rs`
- Add or modify: `crates/leaven-evidence/src/command.rs`
- Add: `crates/leaven-evidence/tests/command.rs`

**Claims:**
- Command records carry command, exit status, bounded inline output or external refs, and duration.
- Agent trajectory evidence can carry command records plus a bounded/ref transcript.
- Evidence remains opaque to `leaven-engine`.

**Implementation Shape:**
- Replace placeholder `command` structs with:
  - `OutputRecord::{Inline { text, truncated }, BlobRef(BlobRef)}`,
  - `CommandRecord { command, exit_status, stdout, stderr, duration }`,
  - `CommandEvidence { records }`,
  - `AgentTrajectoryEvidence { transcript, commands }`.
- Implement `Evidence` for `CommandEvidence` and `AgentTrajectoryEvidence`.
- Add focused tests for inline snippets, blob refs, duration/status preservation, and empty/default shapes.

**Verification:**
- `cargo nextest run -p leaven-evidence --test command`

## Task 3: Add Materializer Contract Tests

**Files:**
- Add: `crates/leaven-engine/tests/materializer_contract.rs`

**Claims:**
- Materializer writes are deterministic for the same graph view and inputs.
- Materializers invoked from a proposer receive proposer-hidden partitions in their read scope and cannot see hidden assessments.
- Cleanup is called exactly once on the successful path.
- `Create + None + informed_by(history)` creates no causal parent edges while preserving bibliographic influence.

**Implementation Shape:**
- Use small local fixtures in the engine test crate.
- Write materialized files into two temp workspace roots and compare bytes.
- Record a hidden partition assessment, build a proposer context with `TrustPolicy::hide_from_proposers`, and assert the materializer cannot see the assessment.
- Use a counting backend whose `cleanup(self)` increments an `Arc<AtomicUsize>`.
- Apply a create proposal informed by a seed candidate and assert `parents(child).is_empty()` while `informed_by(child)` contains the seed.

**Verification:**
- `cargo nextest run -p leaven-engine --test materializer_contract`

## Task 4: Add Workspace Path Property Coverage

**Files:**
- Modify: `crates/leaven-workspace/Cargo.toml`
- Modify: `crates/leaven-workspace/tests/workspace_path.rs`

**Claim:** Generated public workspace paths either normalize to safe relative `/` paths or are refused; accepted paths never expose absolute paths or parent traversal.

**Implementation Shape:**
- Add `proptest` as a dev-dependency.
- Generate small path component vectors without `/`, empty components, `.` or `..` for the accepted case.
- Generate strings containing absolute prefixes, empty components, or parent traversal for refused cases.

**Verification:**
- `cargo nextest run -p leaven-workspace --test workspace_path`

## Task 5: Harden The P4 Example

**Files:**
- Modify: `examples/p4_meta_harness_lite/Cargo.toml`
- Modify: `examples/p4_meta_harness_lite/src/main.rs`

**Claims:**
- The example performs the full Meta-Harness Lite flow from seed to best candidate.
- Materialization is stage-owned and composed by fields.
- The deterministic fake agent reads the materialized workspace and emits a fresh `Create` proposal.
- The evaluator runs in an isolated workspace and returns trajectory evidence through the evidence store.
- Cleanup is awaited exactly once for proposer and evaluator workspaces.

**Implementation Shape:**
- Use `Engine` with a registered deterministic evaluator and `InlineEvidenceStore`.
- Define local `HarnessArtifact`, `HarnessEvidence`, `HistorySnapshot`, `HarnessArtifactMaterializer`, `HarnessEvidenceMaterializer`, and `HistoryMaterializer`.
- Define `AgenticHarnessProposer` owning a workspace factory and history materializer.
- In the proposer:
  - allocate a workspace,
  - materialize `artifact/`, `history/`, and visible `evidence/`,
  - run a deterministic fake agent by reading those files and writing `output/harness_0.py`,
  - return `Proposal::create(...).informed_by(history refs).build()`,
  - always await cleanup before returning.
- In the evaluator:
  - allocate a separate workspace,
  - materialize/evaluate the candidate,
  - return `HarnessEvidence` carrying `AgentTrajectoryEvidence`,
  - await cleanup.
- Optimizer:
  - evaluate seed on search partition,
  - call proposer,
  - apply create proposal,
  - evaluate child on search partition,
  - update `KeepBest`,
  - assert no causal parents for the created child and no hidden validation materialization.

**Verification:**
- `just milestone-p4`

## Task 6: Update Testing Documentation

**Files:**
- Modify: `docs/testing/README.md`

**Claim:** The durable test contract names the P4 suites and direct proof path.

**Implementation Shape:**
- Add `crates/leaven-engine/tests/materializer_contract.rs`.
- Add `crates/leaven-evidence/tests/command.rs`.
- Mention the P4 milestone command beside existing milestone examples.

**Verification:**
- Check referenced paths exist.

## Task 7: Final Verification

**Commands:**

```bash
just milestone-p4
cargo nextest run -p leaven-workspace --test workspace_path
cargo nextest run -p leaven-engine --test materializer_contract
cargo nextest run -p leaven-evidence --test command
just test
just check
```

**Completion Criteria:**
- All commands pass.
- P4 success criteria from the milestone spec are directly tested.
- No coverage floor is lowered.
- No `lib.rs` gains behavior.
- `leaven-core` remains cold and workspace/materializer behavior stays outside it.
- `jj st` shows only intentional P4 plan/test/code/doc changes.
