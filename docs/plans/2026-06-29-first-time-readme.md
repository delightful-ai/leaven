# First-Time README Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Rewrite the root README for first-time users who want to run Leaven from source, use GEPA, optimize Harbor-compatible agent harnesses, or point a coding agent at the right entry point.

**Architecture:** Hard-cut the old Rust-alpha README into a source-checkout product guide. Lead with the Python authoring surface and Harbor compatibility, then route deeper users to GEPA, Harbor/Codex/Claude examples, Rust optimizer seams, and proof labels.

**Tech Stack:** Markdown, Leaven Python SDK examples, Harbor example package, Rust optimizer crates, jj.

---

### Task 1: Replace The Root README

**Files:**
- Modify: `README.md`

**Step 1: Rewrite the first screen**

Replace the Rust-library opening with a first-time-user promise:

- Leaven optimizes things agents can change and measure.
- Users bring a seed artifact, harness/task, rollout, reward/rubric, and optimizer.
- Leaven runs the loop.

**Step 2: Add source-checkout quickstart**

Use the no-spend Python example as the first runnable path:

```bash
git clone https://github.com/delightful-ai/leaven.git
cd leaven/sdk/python
uv sync
just example 03
```

Expected claim: example 03 runs real GEPA over the durable seam with mock LM reflection and asserts the child beats the seed.

**Step 3: Add "Pick Your Path" routing**

Add a table routing users and coding agents to:

- run GEPA
- run live AIME GEPA
- optimize a Harbor-compatible agent harness
- run Codex through Harbor
- run Claude Code through Harbor
- author a new optimizer in Rust
- understand proof labels

**Step 4: Add Harbor compatibility section**

Say that Harbor trials can serve as Leaven rollouts. Name Codex, Claude Code, and any Harbor-compatible agent, while preserving caveats:

- Harbor is optional and private to the example package.
- Live runs require opt-in, credentials, Docker where applicable, and may spend money.
- The live Codex Terminal-Bench path proves served-path function; deterministic no-spend tests prove changed-child mechanics.

**Step 5: Add coding-agent bundle**

Include a copy-paste block that tells a coding agent which repo paths to inspect for:

- GEPA from Python
- Harbor agent-kit optimization
- Codex/Claude Harbor adapters
- new optimizer authoring

**Step 6: Add alpha/proof labels**

State clearly:

- Python SDK is real in-repo source, not a PyPI promise.
- Rust crate is not a crates.io promise.
- Scaffold examples are not product proof.
- Live examples are opt-in.

### Task 2: Verify Markdown And Anchors

**Files:**
- Read: `README.md`

**Step 1: Inspect references**

Run:

```bash
rg -n "sdk/python/examples/03_prompt_optimize.py|15_live_optimize_codex_terminal_bench.py|live_claude_code_trial.py|crates/leaven-engine/src/stage/optimizer.rs|docs/specs/harbor_leaven_adapter.md" README.md
```

Expected: all key anchors appear.

**Step 2: Check repository status**

Run:

```bash
jj status
jj diff --stat
```

Expected: only `README.md` changed for the README implementation commit.

### Task 3: Commit And Publish

**Files:**
- Modify: `README.md`

**Step 1: Describe the README commit**

Use a rich jj message explaining why this hard-cut replaces the old Rust-first story.

**Step 2: Move `main` and push**

Move the `main` bookmark to the README commit and push:

```bash
jj bookmark set main -r @-
jj git push --bookmark main
```

Expected: GitHub `main` contains the new README.
