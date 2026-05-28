# Agent Reflection Smoke Rubric Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a live-gated Codex smoke harness that scores the approved 55-check natural-language rubric for GEPA agent reflection.

**Architecture:** Start in `leaven-gepa-agentic-skill` because it already owns the GEPA reflection bridge over a behavior-bearing `SkillBank` artifact and proves `RunContext::propose` plus `apply_batch`. Keep Codex as a test-only provider leaf via `leaven-agent-codex-cli`; do not move provider protocol, proposal parsing, or hidden-target policy into the bridge crate. The first implementation should include deterministic catalog/fixture tests by default and live Codex scenarios behind `LEAVEN_CODEX_LIVE=1`.

**Tech Stack:** Rust integration tests, `leaven-gepa-agentic-skill`, `leaven-agent-codex-cli`, `leaven-agent`, `leaven-workspace-local`, `serde_json`, ignored live tests.

---

## Task 1: Add Live-Test Feature And Test Dependencies

**Files:**
- Modify: `crates/leaven-gepa-agentic-skill/Cargo.toml`

**Step 1: Add the failing dependency check**

Run:

```bash
cargo test -p leaven-gepa-agentic-skill smoke_rubric_catalog_has_11_stages -- --exact
```

Expected: FAIL because the test does not exist yet.

**Step 2: Add the feature and dev dependencies**

Add:

```toml
[features]
default = []
live-codex-tests = []

[dev-dependencies]
futures = { workspace = true }
leaven-agent-codex-cli = { workspace = true }
leaven-workspace-local = { workspace = true }
serde_json = { workspace = true }
tempfile = { workspace = true }
```

Keep the existing `futures` and `leaven-workspace-local` entries; do not duplicate them.

**Step 3: Verify manifest parses**

Run:

```bash
cargo metadata --no-deps --format-version 1
```

Expected: PASS.

**Step 4: Commit**

```bash
jj describe -m "leaven-gepa-agentic-skill: add live smoke test feature scaffold" && jj new
```

## Task 2: Add Smoke Rubric Catalog Module

**Files:**
- Create: `crates/leaven-gepa-agentic-skill/tests/live_smoke/mod.rs`
- Create: `crates/leaven-gepa-agentic-skill/tests/live_smoke/rubric.rs`
- Create: `crates/leaven-gepa-agentic-skill/tests/agent_reflection_live_smoke.rs`

**Step 1: Write the failing catalog test**

In `agent_reflection_live_smoke.rs`:

```rust
mod live_smoke;

#[test]
fn smoke_rubric_catalog_has_11_stages_and_55_checks() {
    let catalog = live_smoke::rubric::catalog();
    assert_eq!(catalog.len(), 11);
    assert_eq!(catalog.iter().map(|stage| stage.checks.len()).sum::<usize>(), 55);
    assert!(catalog
        .iter()
        .all(|stage| stage.checks.iter().all(|check| !check.text.contains("TODO"))));
}
```

Run:

```bash
cargo test -p leaven-gepa-agentic-skill smoke_rubric_catalog_has_11_stages_and_55_checks -- --exact
```

Expected: FAIL because `live_smoke::rubric` is missing.

**Step 2: Implement the catalog structs and stage list**

In `rubric.rs`:

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SmokeStage {
    pub id: &'static str,
    pub title: &'static str,
    pub checks: Vec<SmokeCheck>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SmokeCheck {
    pub id: &'static str,
    pub text: &'static str,
}

pub fn catalog() -> Vec<SmokeStage> {
    vec![
        stage("parent_candidate_selection", "Parent Candidate Selection", [
            check("parent_candidate_selection.parent_identity_matches_event", "The Codex-visible stage packet names the selected parent candidate id and GEPA candidate index, and those values match the parent-selection event."),
            check("parent_candidate_selection.frontier_evidence_without_targets", "The environment includes enough validation-frontier evidence for Codex to explain why this parent was selectable, without exposing hidden validation targets."),
            check("parent_candidate_selection.decoy_train_best_not_selected", "A decoy train-best candidate that is not validation-frontier selectable is present in the harness evidence, and Codex does not select or cite it as the reflection parent."),
            check("parent_candidate_selection.reason_cites_frontier", "The final stage evidence cites validation-frontier membership or dominance facts, not only a scalar score or best-so-far statement."),
            check("parent_candidate_selection.report_reconstructs_choice", "The report preserves the parent-selection reason so a later scorer can reconstruct the choice without reading transient workspace files."),
        ]),
        // Add the remaining ten stages from docs/plans/2026-05-27-agent-reflection-smoke-rubric-design.md.
    ]
}

const fn check(id: &'static str, text: &'static str) -> SmokeCheck {
    SmokeCheck { id, text }
}

fn stage<const N: usize>(
    id: &'static str,
    title: &'static str,
    checks: [SmokeCheck; N],
) -> SmokeStage {
    SmokeStage {
        id,
        title,
        checks: checks.into_iter().collect(),
    }
}
```

**Step 3: Run the catalog test**

Run:

```bash
cargo test -p leaven-gepa-agentic-skill smoke_rubric_catalog_has_11_stages_and_55_checks -- --exact
```

Expected: PASS.

**Step 4: Commit**

```bash
jj describe -m "leaven-gepa-agentic-skill: add agent reflection smoke rubric catalog" && jj new
```

## Task 3: Add Score Output Parser

**Files:**
- Modify: `crates/leaven-gepa-agentic-skill/tests/live_smoke/mod.rs`
- Create: `crates/leaven-gepa-agentic-skill/tests/live_smoke/score.rs`
- Modify: `crates/leaven-gepa-agentic-skill/tests/agent_reflection_live_smoke.rs`

**Step 1: Write failing parser tests**

Add tests:

```rust
#[test]
fn smoke_score_rejects_missing_evidence_refs() {
    let json = serde_json::json!({
        "stage": "parent_candidate_selection",
        "checks": [{
            "id": "parent_candidate_selection.parent_identity_matches_event",
            "status": "pass",
            "evidence_refs": [],
            "notes": "looks good"
        }]
    });

    let err = live_smoke::score::StageScore::parse(&json).unwrap_err();
    assert!(err.to_string().contains("evidence_refs"));
}
```

Run:

```bash
cargo test -p leaven-gepa-agentic-skill smoke_score_rejects_missing_evidence_refs -- --exact
```

Expected: FAIL because parser is missing.

**Step 2: Implement strict pass/fail parsing**

Implement:

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckStatus {
    Pass,
    Fail,
}

#[derive(Clone, Debug)]
pub struct CheckScore {
    pub id: String,
    pub status: CheckStatus,
    pub evidence_refs: Vec<String>,
    pub notes: String,
}

#[derive(Clone, Debug)]
pub struct StageScore {
    pub stage: String,
    pub checks: Vec<CheckScore>,
}
```

Rules:
- reject unknown status values;
- reject pass with empty evidence refs;
- reject unknown check ids;
- reject duplicate check ids;
- require exactly five checks for the stage.

**Step 3: Run parser tests**

Run:

```bash
cargo test -p leaven-gepa-agentic-skill smoke_score -- --nocapture
```

Expected: PASS for parser tests.

**Step 4: Commit**

```bash
jj describe -m "leaven-gepa-agentic-skill: parse scored smoke rubric output" && jj new
```

## Task 4: Add Deterministic Stage Fixture Builder

**Files:**
- Create: `crates/leaven-gepa-agentic-skill/tests/live_smoke/fixture.rs`
- Modify: `crates/leaven-gepa-agentic-skill/tests/live_smoke/mod.rs`
- Modify: `crates/leaven-gepa-agentic-skill/tests/agent_reflection_live_smoke.rs`

**Step 1: Write failing fixture tests**

Add:

```rust
#[test]
fn fixture_keeps_hidden_targets_out_of_reflector_workspace() {
    let fixture = live_smoke::fixture::SmokeFixture::new("reflective_dataset_construction");
    let files = fixture.materialized_files();

    assert!(files.iter().any(|path| path.ends_with("reflection/examples.json")));
    assert!(!files.iter().any(|path| path.contains("hidden-targets")));
    assert!(!fixture.visible_text().contains("case.target"));
}
```

Run:

```bash
cargo test -p leaven-gepa-agentic-skill fixture_keeps_hidden_targets_out_of_reflector_workspace -- --exact
```

Expected: FAIL because fixture module is missing.

**Step 2: Implement fixture builder**

Fixture must create:
- one parent `SkillBank` with selected `alpha/SKILL.md`;
- one decoy skill outside the selected part;
- train minibatch cases and a decoy train case;
- hidden validation/test target material stored only in scorer-private test data, not materialized to reflector workspace;
- stage manifest JSON with run id, attempt id, parent id, stage id, selected part, and expected output path.

**Step 3: Run fixture tests**

Run:

```bash
cargo test -p leaven-gepa-agentic-skill fixture_ -- --nocapture
```

Expected: PASS.

**Step 4: Commit**

```bash
jj describe -m "leaven-gepa-agentic-skill: build target-safe smoke fixtures" && jj new
```

## Task 5: Add Live Codex Runner Helper

**Files:**
- Create: `crates/leaven-gepa-agentic-skill/tests/live_smoke/codex.rs`
- Modify: `crates/leaven-gepa-agentic-skill/tests/live_smoke/mod.rs`
- Modify: `crates/leaven-gepa-agentic-skill/tests/agent_reflection_live_smoke.rs`

**Step 1: Write ignored live test skeleton**

Add:

```rust
#[test]
#[ignore = "requires local Codex auth and LEAVEN_CODEX_LIVE=1"]
#[cfg(feature = "live-codex-tests")]
fn live_codex_smoke_runner_writes_score_file() {
    if std::env::var("LEAVEN_CODEX_LIVE").as_deref() != Ok("1") {
        eprintln!("skipping live Codex test because LEAVEN_CODEX_LIVE != 1");
        return;
    }
    futures::executor::block_on(async {
        let fixture = live_smoke::fixture::SmokeFixture::new("live_codex_reflection_session");
        let score = live_smoke::codex::run_stage(&fixture).await.unwrap();
        assert_eq!(score.stage, "live_codex_reflection_session");
        assert_eq!(score.checks.len(), 5);
    });
}
```

Run:

```bash
cargo test -p leaven-gepa-agentic-skill --features live-codex-tests live_codex_smoke_runner_writes_score_file -- --ignored --exact
```

Expected: If `LEAVEN_CODEX_LIVE` is not set, the test prints a skip message and passes.

**Step 2: Implement `run_stage`**

Use `CodexCliRuntime` with:
- `LEAVEN_CODEX_BIN` override or `$HOME/.bun/bin/codex`;
- `CodexCliReasoningEffort::Low`;
- explicit workspace-write or bypass mode only if the fixture workspace is the sandbox boundary;
- `OutputContract::JsonFile { path: "output/smoke-score.json" }`.

Prompt shape:

```text
You are verifying one Leaven GEPA agent-reflection stage.
Read stage/MANIFEST.json and the stage evidence files.
Answer by writing output/smoke-score.json.
For each rubric check, return pass or fail and cite concrete evidence refs.
Do not use hidden targets. Do not modify files outside output/.
```

**Step 3: Run no-live gate**

Run:

```bash
cargo test -p leaven-gepa-agentic-skill --features live-codex-tests live_codex_smoke_runner_writes_score_file -- --ignored --exact
```

Expected without `LEAVEN_CODEX_LIVE=1`: PASS with skip message.

**Step 4: Commit**

```bash
jj describe -m "leaven-gepa-agentic-skill: scaffold live Codex smoke runner" && jj new
```

## Task 6: Add The Eleven Stage Scenario Builders

**Files:**
- Modify: `crates/leaven-gepa-agentic-skill/tests/live_smoke/fixture.rs`
- Modify: `crates/leaven-gepa-agentic-skill/tests/agent_reflection_live_smoke.rs`

**Step 1: Write failing catalog-to-fixture coverage test**

Add:

```rust
#[test]
fn every_rubric_stage_has_a_fixture() {
    for stage in live_smoke::rubric::catalog() {
        let fixture = live_smoke::fixture::SmokeFixture::new(stage.id);
        assert_eq!(fixture.stage_id(), stage.id);
        assert!(fixture.materialized_files().iter().any(|path| path.ends_with("stage/MANIFEST.json")));
    }
}
```

Run:

```bash
cargo test -p leaven-gepa-agentic-skill every_rubric_stage_has_a_fixture -- --exact
```

Expected: FAIL until all eleven stage ids are supported.

**Step 2: Implement fixture variants**

Add fixture setup for:
- `parent_candidate_selection`
- `train_minibatch_binding`
- `parent_evaluation_evidence`
- `skip_gates_and_part_selection`
- `reflective_dataset_construction`
- `agent_workspace_materialization`
- `reflection_instruction_output_contract`
- `live_codex_reflection_session`
- `workspace_readback_typed_change`
- `proposal_recording_graph_application`
- `child_screening_validation_report_checkpoint`

Each fixture must include a plausible decoy or failure mode for at least one check.

**Step 3: Run fixture coverage**

Run:

```bash
cargo test -p leaven-gepa-agentic-skill every_rubric_stage_has_a_fixture -- --exact
```

Expected: PASS.

**Step 4: Commit**

```bash
jj describe -m "leaven-gepa-agentic-skill: cover all agent reflection smoke stages" && jj new
```

## Task 7: Add Full Live Matrix Test

**Files:**
- Modify: `crates/leaven-gepa-agentic-skill/tests/agent_reflection_live_smoke.rs`

**Step 1: Write the ignored live matrix test**

Add:

```rust
#[test]
#[ignore = "requires local Codex auth and LEAVEN_CODEX_LIVE=1"]
#[cfg(feature = "live-codex-tests")]
fn live_codex_scores_all_agent_reflection_smoke_stages() {
    if std::env::var("LEAVEN_CODEX_LIVE").as_deref() != Ok("1") {
        eprintln!("skipping live Codex matrix because LEAVEN_CODEX_LIVE != 1");
        return;
    }

    futures::executor::block_on(async {
        let mut total = 0usize;
        for stage in live_smoke::rubric::catalog() {
            let fixture = live_smoke::fixture::SmokeFixture::new(stage.id);
            let score = live_smoke::codex::run_stage(&fixture).await.unwrap();
            live_smoke::score::assert_stage_score(&stage, &score).unwrap();
            total += score.checks.iter().filter(|check| check.is_pass()).count();
        }
        assert_eq!(total, 55);
    });
}
```

Run:

```bash
cargo test -p leaven-gepa-agentic-skill --features live-codex-tests live_codex_scores_all_agent_reflection_smoke_stages -- --ignored --exact
```

Expected without `LEAVEN_CODEX_LIVE=1`: PASS with skip message.
Expected with `LEAVEN_CODEX_LIVE=1`: 11 live Codex sessions and PASS only if all 55 checks pass.

**Step 2: Commit**

```bash
jj describe -m "leaven-gepa-agentic-skill: add live Codex reflection smoke matrix" && jj new
```

## Task 8: Update Local Ownership Docs

**Files:**
- Modify: `crates/leaven-gepa-agentic-skill/AGENTS.md`
- Modify: `docs/plans/2026-05-27-agent-reflection-smoke-rubric-design.md`

**Step 1: Add local live-smoke guidance**

In `crates/leaven-gepa-agentic-skill/AGENTS.md`, add a proof anchor:

```markdown
- `crates/leaven-gepa-agentic-skill/tests/agent_reflection_live_smoke.rs`
  owns the live-gated Codex smoke rubric for GEPA skill-bank agent reflection.
  It proves environment-rights and stage evidence over a real Codex run, not
  paper parity or generic Codex provider behavior.
```

Add verification:

```markdown
- `LEAVEN_CODEX_LIVE=1 cargo test -p leaven-gepa-agentic-skill --features live-codex-tests --test agent_reflection_live_smoke -- --ignored`
  spends local Codex auth/runtime and proves the 11-stage agent-reflection smoke matrix.
```

**Step 2: Mark the design doc implemented or partially implemented**

Add a short status note to the design doc after the header:

```markdown
Implementation status: scaffolded by `tests/agent_reflection_live_smoke.rs`.
The suite is live-gated and does not run in default `just test`.
```

**Step 3: Commit**

```bash
jj describe -m "docs: route agent reflection live smoke ownership" && jj new
```

## Task 9: Run Verification Gates

**Files:**
- No edits unless failures require fixes.

**Step 1: Default deterministic gate**

Run:

```bash
cargo test -p leaven-gepa-agentic-skill
```

Expected: PASS.

**Step 2: Live feature compile and skip gate**

Run:

```bash
cargo test -p leaven-gepa-agentic-skill --features live-codex-tests --test agent_reflection_live_smoke -- --ignored
```

Expected without `LEAVEN_CODEX_LIVE=1`: PASS with skip messages.

**Step 3: Optional paid/live gate**

Run only with explicit operator intent and local Codex auth:

```bash
LEAVEN_CODEX_LIVE=1 cargo test -p leaven-gepa-agentic-skill --features live-codex-tests --test agent_reflection_live_smoke -- --ignored --nocapture
```

Expected: 11 live Codex sessions, 55 pass-scored checks.

**Step 4: Topology gate if dependencies or features changed cross-crate**

Run:

```bash
cargo test -p leaven --test topology_contract
```

Expected: PASS.

**Step 5: Completion gate**

Run:

```bash
just check
```

Expected: PASS before claiming implementation complete.

**Step 6: Final commit if fixes were needed**

```bash
jj describe -m "leaven-gepa-agentic-skill: verify live agent reflection smoke rubric" && jj new
```
