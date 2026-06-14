# Trace2Skill ARA Reproduction Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build an ARA-governed Trace2Skill / SpreadsheetBench reproduction path that turns paper/source evidence into plots, then advances Leaven from no-spend mechanics to honest paper-denominator replication.

**Architecture:** Compile the Trace2Skill paper, upstream repository, and Leaven replication dossier into an Agent-Native Research Artifact under `docs/ara/trace2skill_spreadsheetbench/`. Use that ARA as the single reproduction denominator for claims, evidence tables, plot data, execution stubs, and the Leaven implementation gap list. Only after the ARA validates do code changes advance `examples/trace2skill_spreadsheetbench` or live execution.

**Tech Stack:** ARA compiler schema, Leaven Rust workspace, `examples/trace2skill_spreadsheetbench`, `examples/trace2skill_tiny_live`, upstream Trace2Skill Python, SpreadsheetBench-Verified assets, `uv run --with` for one-off Python plotting/validation tools, `jj` for frequent snapshots.

---

## Intent Check

Original intent: reproduce an agent harness optimization paper where the optimized object is the agent skill/harness, not merely a prompt.

Chosen paper: Trace2Skill / SpreadsheetBench, because the paper optimizes a portable skill directory through trajectory generation, analyst patch proposal, and hierarchical consolidation.

Do not substitute these proxies for success:
- The existing `trace2skill_tiny_live` CSV loop is useful causal proof, but it is not SpreadsheetBench/Qwen/vLLM parity.
- The existing `trace2skill_spreadsheetbench` tests prove manifest, patch, replay, and scorer mechanics, but they do not prove live analyst calls, live merge calls, or paper metric reproduction.
- A plot recreated from paper tables is not a Leaven result. It is the target line we must match.
- One passing case `13-1` is not the held-out `200..400` paper denominator.

Acceptance at the end of the full program:
- A valid ARA exists and records exact Trace2Skill claims, tables, configs, algorithms, prompt/materialization facts, source anchors, and evidence limits.
- Leaven can regenerate the plot sheet from ARA evidence and overlay Leaven-run results against paper targets.
- Leaven can run, score, and record at least the staged `13-1` SpreadsheetBench case through the intended seam without a fake success envelope.
- Any claim of paper reproduction names the actual denominator: one-case mechanics, subset live run, full held-out split, or full cross-model/cross-domain paper table.

## Source Denominator

Use these inputs for the ARA compiler pass:
- Paper/source bundle: `tmp/skill_opt_sources/arx_2603.25158/`
- Main extracted source: `tmp/skill_opt_sources/arx_2603.25158/full_source.md`
- Upstream repo checkout: `tmp/repros/trace2skill-upstream/`
- Vendor checkout: `/Users/darin/vendor/github.com/Qwen-Applications/Trace2Skill/`
- Leaven dossier: `docs/working-memory/trace2skill-replication.md`
- Leaven maturity summary: `docs/working-memory/skill-paper-replication.md`
- Mechanics example: `examples/trace2skill_spreadsheetbench/`
- Tiny live proxy: `examples/trace2skill_tiny_live/`

Paper anchors already known:
- Three stages: trajectory generation, parallel analyst patch proposal, conflict-free hierarchical consolidation.
- SpreadsheetBench-Verified: 400 rows, `0..200` evolving/train and `200..400` held-out, seeds `41/42/43`.
- Model/harness settings: Qwen3.5-122B-A10B and Qwen3.5-35B-A3B served through vLLM, Stage 2 uses 128 sub-agents, merge batch size 32, ReAct turn budget 100.
- Efficiency target: parallel consolidation reports about `3 min` versus sequential `15 min` and `60 min` baselines on an 8-GPU A800 node.

## Task 1: Create the Trace2Skill ARA Skeleton

**Files:**
- Create: `docs/ara/trace2skill_spreadsheetbench/PAPER.md`
- Create: `docs/ara/trace2skill_spreadsheetbench/logic/problem.md`
- Create: `docs/ara/trace2skill_spreadsheetbench/logic/claims.md`
- Create: `docs/ara/trace2skill_spreadsheetbench/logic/concepts.md`
- Create: `docs/ara/trace2skill_spreadsheetbench/logic/experiments.md`
- Create: `docs/ara/trace2skill_spreadsheetbench/logic/solution/architecture.md`
- Create: `docs/ara/trace2skill_spreadsheetbench/logic/solution/algorithm.md`
- Create: `docs/ara/trace2skill_spreadsheetbench/logic/solution/constraints.md`
- Create: `docs/ara/trace2skill_spreadsheetbench/logic/solution/heuristics.md`
- Create: `docs/ara/trace2skill_spreadsheetbench/logic/related_work.md`
- Create: `docs/ara/trace2skill_spreadsheetbench/src/configs/training.md`
- Create: `docs/ara/trace2skill_spreadsheetbench/src/configs/model.md`
- Create: `docs/ara/trace2skill_spreadsheetbench/src/execution/trace2skill_pipeline.py`
- Create: `docs/ara/trace2skill_spreadsheetbench/src/environment.md`
- Create: `docs/ara/trace2skill_spreadsheetbench/trace/exploration_tree.yaml`
- Create: `docs/ara/trace2skill_spreadsheetbench/evidence/README.md`

**Step 1: Make directories**

Run:

```bash
mkdir -p docs/ara/trace2skill_spreadsheetbench/{logic/solution,src/configs,src/execution,trace,evidence/tables,evidence/figures}
```

Expected: directories exist.

**Step 2: Compile the first-pass ARA**

Read the source denominator and fill every mandatory file from the ARA compiler schema. Preserve exact numbers in evidence only; keep `logic/experiments.md` directional.

Expected: every mandatory ARA file exists and is non-trivial.

**Step 3: Commit**

Run:

```bash
jj describe -m "docs: add Trace2Skill ARA skeleton"
jj new
```

## Task 2: Transcribe Paper Tables and Figure Evidence

**Files:**
- Create: `docs/ara/trace2skill_spreadsheetbench/evidence/tables/table_main_spreadsheetbench.md`
- Create: `docs/ara/trace2skill_spreadsheetbench/evidence/tables/table_parallel_vs_sequential.md`
- Create: `docs/ara/trace2skill_spreadsheetbench/evidence/tables/table_reasoningbank.md`
- Create: `docs/ara/trace2skill_spreadsheetbench/evidence/figures/figure_trace2skill_framework.md`
- Modify: `docs/ara/trace2skill_spreadsheetbench/evidence/README.md`
- Modify: `docs/ara/trace2skill_spreadsheetbench/logic/claims.md`

**Step 1: Write faithful raw evidence files**

Transcribe source tables from `tmp/skill_opt_sources/arx_2603.25158/full_source.md` without filtering rows. If a view is curated for plotting, name it `derived_...`, not `table_...`.

Expected: exact source captions, source line/path, and cell values are preserved.

**Step 2: Connect evidence to claims**

Update `claims.md` so claims reference experiment IDs, and update `evidence/README.md` so every evidence file maps to the claims it supports.

Expected: no claim cites an evidence table that lacks the compared rows.

**Step 3: Commit**

Run:

```bash
jj describe -m "docs: transcribe Trace2Skill ARA evidence tables"
jj new
```

## Task 3: Add ARA Seal Level 1 Validation

**Files:**
- Create: `scripts/validate_ara.py`
- Create: `docs/ara/trace2skill_spreadsheetbench/validation.md`

**Step 1: Write a small validator**

Implement only structural checks first:
- mandatory directories exist;
- mandatory files exist and are non-empty;
- `PAPER.md` frontmatter has title/authors/year;
- `claims.md` has `C01`;
- `experiments.md` has `E01`;
- `concepts.md` has at least five `##` concepts;
- `exploration_tree.yaml` parses and has at least eight nodes;
- evidence files contain `Source`.

Use inline dependencies:

```bash
uv run --with pyyaml python scripts/validate_ara.py docs/ara/trace2skill_spreadsheetbench
```

Expected initially: fails on any missing fields.

**Step 2: Fix the ARA until validation passes**

Patch ARA files, not the validator, unless the validator check is wrong.

Expected: validator exits `0` and writes a short result into `validation.md`.

**Step 3: Commit**

Run:

```bash
jj describe -m "scripts: validate Trace2Skill ARA structure"
jj new
```

## Task 4: Generate Plot Targets From ARA Evidence

**Files:**
- Create: `scripts/plot_trace2skill_ara.py`
- Create: `docs/ara/trace2skill_spreadsheetbench/plots/README.md`
- Generate: `docs/ara/trace2skill_spreadsheetbench/plots/trace2skill_targets.png`

**Step 1: Write the plotting script**

Read the ARA evidence tables and produce:
- baseline skill vs evolved skill;
- average improvement by author/mode/condition;
- parallel vs sequential Vrf and runtime;
- Trace2Skill skill vs ReasoningBank.

Run:

```bash
uv run --with matplotlib --with pandas python scripts/plot_trace2skill_ara.py docs/ara/trace2skill_spreadsheetbench
```

Expected: `trace2skill_targets.png` is generated from ARA evidence, not hard-coded from a scratch script.

**Step 2: Document plot meaning**

In `plots/README.md`, state that the figure is a paper target sheet. It is not a Leaven reproduction result until Leaven result overlays are added.

**Step 3: Commit**

Run:

```bash
jj describe -m "scripts: plot Trace2Skill paper targets from ARA"
jj new
```

## Task 5: Connect Existing Leaven Mechanics to ARA Claims

**Files:**
- Modify: `docs/ara/trace2skill_spreadsheetbench/logic/experiments.md`
- Modify: `docs/ara/trace2skill_spreadsheetbench/logic/claims.md`
- Modify: `docs/ara/trace2skill_spreadsheetbench/evidence/README.md`
- Modify if needed: `examples/trace2skill_spreadsheetbench/tests/*.rs`

**Step 1: Classify current tests as evidence level**

Map existing tests to the ARA:
- `manifest`: paper row-order and split mechanics;
- `run_artifacts`: upstream run artifact lowering;
- `patch_bridge`: patch lowering/application mechanics;
- `patch_replay`: saved Stage 2/3 merge replay mechanics;
- `one_case` / `cli`: `13-1` no-spend prompt and scorer;
- `workbook_score`: exact answer-range comparison;
- `acp_external_worker`: deterministic external-worker mechanics.

Expected: each test is classified as mechanics-smoke, one-case proof, or paper-denominator proof.

**Step 2: Run focused tests**

Run:

```bash
cargo test -p trace2skill_spreadsheetbench --test manifest
cargo test -p trace2skill_spreadsheetbench --test run_artifacts
cargo test -p trace2skill_spreadsheetbench --test patch_bridge
cargo test -p trace2skill_spreadsheetbench --test patch_replay
cargo test -p trace2skill_spreadsheetbench --test one_case --test cli
cargo test -p trace2skill_spreadsheetbench --test workbook_score
cargo test -p trace2skill_spreadsheetbench --test acp_external_worker
```

Expected: tests pass, or the ARA records the exact blocker without upgrading the claim.

**Step 3: Commit**

Run:

```bash
jj describe -m "docs: bind Trace2Skill mechanics tests to ARA claims"
jj new
```

## Task 6: Add Leaven Result Overlay Format

**Files:**
- Create: `docs/ara/trace2skill_spreadsheetbench/results/leaven_result_schema.md`
- Create: `docs/ara/trace2skill_spreadsheetbench/results/README.md`
- Modify: `scripts/plot_trace2skill_ara.py`

**Step 1: Define result JSON schema**

Define the minimal result records needed for overlays:
- run id;
- proof classification;
- dataset slice;
- model id;
- seed;
- skill source;
- metric name;
- metric value;
- cost/runtime fields;
- source command;
- artifact paths.

Expected: schema can represent paper target rows and Leaven result rows without mixing them.

**Step 2: Add overlay plotting**

Teach the plot script to optionally read `results/*.jsonl` and draw Leaven points against paper target bars/lines.

Expected: with no results, paper target plots still render; with result JSONL, overlays render.

**Step 3: Commit**

Run:

```bash
jj describe -m "docs: define Trace2Skill Leaven result overlays"
jj new
```

## Task 7: Advance From One-Case Mechanics to One-Case Live Proof

**Files:**
- Modify if needed: `examples/trace2skill_spreadsheetbench/src/*.rs`
- Modify if needed: `examples/trace2skill_spreadsheetbench/tests/*.rs`
- Create: `docs/ara/trace2skill_spreadsheetbench/results/one_case_live.md`

**Step 1: Prepare the one-case run**

Run:

```bash
cargo run -p trace2skill_spreadsheetbench -- --prepare-one-case-run --run-dir tmp/trace2skill-one-case-live
```

Expected: run directory contains `agent_prompt.md`, staged workbooks, manifest, and deterministic output path.

**Step 2: Execute through the approved live worker path**

Use the existing external-worker / ACP path first. If a live model path is introduced, it must be opt-in and recorded with model id, command, transcript, spend/risk note, and output workbook path.

Expected: success requires a real output workbook plus valid result envelope; envelope-only success remains failure.

**Step 3: Score and record evidence**

Run:

```bash
cargo run -p trace2skill_spreadsheetbench -- --score-one-case-run --run-dir tmp/trace2skill-one-case-live --model-id <id> --transcript-file <path>
```

Expected: `score_report.json`, `manifest.json`, and `trajectory.json` are written and referenced from ARA result records.

**Step 4: Commit**

Run:

```bash
jj describe -m "examples: record Trace2Skill one-case live proof"
jj new
```

## Task 8: Plan the Full Paper-Denominator Run

**Files:**
- Create: `docs/ara/trace2skill_spreadsheetbench/results/full_run_plan.md`
- Modify: `docs/ara/trace2skill_spreadsheetbench/logic/experiments.md`
- Modify: `docs/ara/trace2skill_spreadsheetbench/src/environment.md`

**Step 1: Write the full-run compute and dependency plan**

Record exact requirements before running:
- Qwen3.5-122B-A10B / Qwen3.5-35B-A3B availability;
- vLLM serving shape;
- 400-row dataset path;
- seeds `41/42/43`;
- workers `128`;
- merge batch size `32`;
- ReAct turn budget `100`;
- expected runtime/cost envelope;
- credentials and hardware approvals.

Expected: user can approve or reject the actual spend/compute plan.

**Step 2: Define subset gates**

Use honest gates:
- `1` case: live seam and scorer;
- `N` cases: trajectory import and analyst fan-out sanity;
- `0..200`: evolving/train split;
- `200..400`: held-out paper split;
- seeds `41/42/43`: paper aggregation.

Expected: no subset gate is named “paper reproduced.”

**Step 3: Commit**

Run:

```bash
jj describe -m "docs: plan Trace2Skill full paper denominator"
jj new
```

## Task 9: Review With ARA Rigor

**Files:**
- Create: `docs/ara/trace2skill_spreadsheetbench/reviews/rigor_review.md`

**Step 1: Run the ARA rigor pass**

Use the ARA reviewer lens after Level 1 passes:
- evidence sufficiency;
- claim strength;
- proxy substitution;
- missing configs;
- missing paper prompts;
- drift between upstream code and paper text;
- result-denominator honesty.

Expected: `rigor_review.md` lists blockers before any “reproduced” claim.

**Step 2: Commit**

Run:

```bash
jj describe -m "docs: review Trace2Skill ARA reproduction rigor"
jj new
```

## Verification Policy

Default focused verification:

```bash
uv run --with pyyaml python scripts/validate_ara.py docs/ara/trace2skill_spreadsheetbench
uv run --with matplotlib --with pandas python scripts/plot_trace2skill_ara.py docs/ara/trace2skill_spreadsheetbench
cargo test -p trace2skill_spreadsheetbench --test manifest
cargo test -p trace2skill_spreadsheetbench --test run_artifacts
cargo test -p trace2skill_spreadsheetbench --test patch_bridge
cargo test -p trace2skill_spreadsheetbench --test patch_replay
cargo test -p trace2skill_spreadsheetbench --test one_case --test cli
cargo test -p trace2skill_spreadsheetbench --test workbook_score
cargo test -p trace2skill_spreadsheetbench --test acp_external_worker
```

Escalate to `just check` only after changing shared crates, default surfaces, workspace test tooling, or public proof classification.

## Stop Conditions

Stop and ask before:
- launching Qwen/vLLM-scale runs;
- running live provider/model calls beyond the existing explicit opt-in lanes;
- claiming more than one-case or mechanics proof;
- adding a new crate or new child `AGENTS.md`;
- converting this into a Harbor task path instead of the paper’s SpreadsheetBench path.

## Open Decision

First execution tranche should be Task 1 through Task 4: compile the ARA, validate it, and generate the paper-target plots from ARA evidence. That gives us the scoreboard before we touch live execution.
