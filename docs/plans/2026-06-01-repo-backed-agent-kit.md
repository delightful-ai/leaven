# Repo-Backed AgentKit Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Make first-class Codex optimization flow through a provider-neutral, repo-backed `AgentKit` whose durable identity is a VCS revision, with Codex as a materialization profile.

**Architecture:** The optimized artifact remains a `GitProgramArtifact`/`GitRepoArtifact` revision. `AgentKit` is a typed semantic view over a repo subtree: `manifest.toml`, `system_prompt.md`, `AGENTS.md`, `skills/`, and later `hooks/`/`harness/`. Codex support is a light profile that projects those slots into Codex's workspace ABI, such as root `AGENTS.md`, `.agents/skills`, and the runtime instruction channel.

**Tech Stack:** Rust workspace crates, `leaven-artifact-git`, `leaven-agentic-git`, `leaven-agent-codex-cli`, `leaven-artifact-skill`, `toml`, `serde`, `jj`.

---

## Design

### Decision

Use a provider-neutral `AgentKit` view backed by repo identity.

```text
GitProgramArtifact / GitRepoArtifact
  repo revision = durable artifact identity

AgentKit view
  manifest.toml
  system_prompt.md
  AGENTS.md
  skills/
  hooks/        # scaffold only in first slice
  harness/      # optional; absent for simple Codex kits

Codex profile
  system_prompt.md -> Codex system/base instruction channel
  AGENTS.md -> workspace/AGENTS.md
  skills/ -> workspace/.agents/skills
```

`CodexKit` should be a profile or convenience wrapper over `AgentKit`, not a separate artifact identity. The Codex layout is operational projection; the repo revision is identity.

### File Meanings

`system_prompt.md` is first-class candidate state. If the optimizer changes it, candidate behavior changes. It must not be hidden as runtime config.

`AGENTS.md` is first-class candidate state when it is part of the kit. It carries durable agent-facing behavior rules.

`manifest.toml` is Leaven-facing structure, not model-facing prose. It names kit slots, profile mappings, schema version, and optional validation settings.

`skills/` is an Agent Skills bank. The initial implementation should reuse `leaven-artifact-skill` validation instead of creating parallel skill parsing.

`hooks/` is scaffold only for the first slice. The manifest may reserve the slot, but no hook execution semantics should ship until the hook law is designed.

`harness/` is optional runnable glue owned by the candidate. Codex-first slice can omit it. It becomes necessary when the candidate includes a custom runner, normalizer, or task application.

### Rejected Alternatives

1. Pure `CodexKit` artifact:
   This would land quickly, but it bakes Codex layout into artifact identity and makes `.agents/skills` feel semantic. That contradicts the existing skill layout rule: provider mount paths are projection, not identity.

2. Pure structured `AgentKit` blob:
   This is clean, but loses repo-native identity, commit history, branching, external authoring, and easy inspection. Leaven already has repo artifact identity; the kit should use it.

3. Repo-only with no typed kit view:
   This preserves identity but gives Leaven no way to validate kit shape, target `system_prompt.md` separately, or produce profile-specific materialization without ad hoc path conventions.

## Proposed First Slice

Ship a repo-backed AgentKit contract for Codex with:

- `manifest.toml`
- `system_prompt.md`
- `AGENTS.md`
- `skills/`
- `hooks/` reserved as scaffold only
- no harness requirement
- Codex CLI profile materialization
- typed validation and readback over repo changes

Do not add hook execution. Do not add a separate Codex artifact. Do not expose this through ordinary prelude/default-feature routes until a live Codex proof exists.

---

### Task 1: Promote The Design Into The Owning Spec

**Files:**
- Modify: `docs/specs/agentic_stage_runtime.md`
- Modify: `docs/specs/agentic_reflection.md`
- Modify: `crates/leaven-artifact-git/AGENTS.md`
- Modify: `crates/leaven-agentic-git/AGENTS.md`

**Step 1: Update `agentic_stage_runtime.md`**

Add a subsection under "Composite agent artifacts" that states:

```markdown
For repo-backed agent kits, the durable artifact identity is the underlying
`GitProgramArtifact` or `GitRepoArtifact` revision. `AgentKit` is the typed
semantic view over a repo subtree, not a replacement identity.

The first Codex profile recognizes `manifest.toml`, `system_prompt.md`,
`AGENTS.md`, and `skills/`. `hooks/` is reserved scaffold only until hook
execution has a typed law and tests. `harness/` is optional and absent for
simple Codex kits.
```

**Step 2: Update `agentic_reflection.md`**

Replace the open "Composite agent kits" question with the chosen first slice:

```markdown
Composite agent kits arrive as a repo-backed AgentKit view over
`GitProgramArtifact`, with Codex as the first materialization profile.
```

**Step 3: Update crate routing docs**

In `leaven-artifact-git/AGENTS.md`, clarify that repo identity remains here and kit validation does not.

In `leaven-agentic-git/AGENTS.md`, clarify that Codex profile materialization may compose Git checkout plus kit projection, but provider flags still belong in provider leaves.

**Step 4: Verify doc references**

Run:

```bash
rg -n "AgentKit|system_prompt.md|hooks/|harness/" docs/specs/agentic_stage_runtime.md docs/specs/agentic_reflection.md crates/leaven-artifact-git/AGENTS.md crates/leaven-agentic-git/AGENTS.md
```

Expected: every new term appears in an owning context; no text claims hook execution exists.

**Step 5: Commit**

```bash
jj describe -m "docs: settle repo-backed AgentKit identity"
jj new
```

### Task 2: Add The AgentKit Contract Crate

**Files:**
- Modify: `Cargo.toml`
- Modify: `crates/leaven/tests/topology_contract.rs`
- Create: `crates/leaven-artifact-agent-kit/Cargo.toml`
- Create: `crates/leaven-artifact-agent-kit/AGENTS.md`
- Create: `crates/leaven-artifact-agent-kit/src/lib.rs`
- Create: `crates/leaven-artifact-agent-kit/src/manifest.rs`
- Create: `crates/leaven-artifact-agent-kit/src/path.rs`
- Create: `crates/leaven-artifact-agent-kit/src/profile.rs`
- Create: `crates/leaven-artifact-agent-kit/tests/agent_kit_contract.rs`

**Step 1: Write failing topology contract update**

Add `crates/leaven-artifact-agent-kit` and `leaven-artifact-agent-kit` to the topology contract.

Expected dependencies:

```text
leaven-artifact-agent-kit -> leaven-artifact-git
leaven-artifact-agent-kit -> leaven-artifact-skill
leaven-artifact-agent-kit -> leaven-kernel
leaven-artifact-agent-kit -> leaven-workspace
```

If `leaven-workspace` is only needed for path vocabulary, consider using `leaven-kernel`/local path newtypes instead. Do not depend on `leaven-agent`, `leaven-agentic`, or `leaven-agent-codex-*`.

Run:

```bash
cargo test -p leaven --test topology_contract
```

Expected: FAIL because the crate does not exist yet.

**Step 2: Create crate scaffold**

Add a crate that owns:

```rust
pub struct AgentKitManifest {
    pub schema: AgentKitSchema,
    pub system_prompt: Option<AgentKitPath>,
    pub agent_docs: Option<AgentKitPath>,
    pub skills: Option<AgentKitPath>,
    pub hooks: Option<AgentKitPath>,
    pub harness: Option<AgentKitPath>,
    pub profiles: AgentKitProfiles,
}

pub struct AgentKitProfileCodex {
    pub system_prompt_channel: CodexSystemPromptChannel,
    pub agent_docs_mount: AgentKitPath,
    pub skills_mount: AgentKitPath,
}

pub enum CodexSystemPromptChannel {
    BaseInstructions,
    StdinPreamble,
}
```

**Step 3: Write manifest validation tests**

In `agent_kit_contract.rs`, prove:

- `manifest.toml` with `system_prompt.md`, `AGENTS.md`, and `skills/` parses.
- missing all behavior-bearing slots is rejected.
- `hooks/` may be declared but is marked scaffold-only.
- absolute paths and escaping paths are rejected.
- Codex profile defaults to `AGENTS.md` and `.agents/skills`.

Run:

```bash
cargo test -p leaven-artifact-agent-kit --test agent_kit_contract
```

Expected: FAIL until implementation exists.

**Step 4: Implement minimal parser and validators**

Implement only the fields above. Preserve unknown manifest fields only if the chosen TOML parser supports a metadata bag without hiding typos in known sections. Otherwise reject unknown fields for slice one.

**Step 5: Verify**

Run:

```bash
cargo test -p leaven-artifact-agent-kit
cargo test -p leaven --test topology_contract
```

Expected: PASS.

**Step 6: Commit**

```bash
jj describe -m "leaven-artifact-agent-kit: add repo-backed kit manifest contract"
jj new
```

### Task 3: Add Codex Profile Materialization

**Files:**
- Create: `crates/leaven-agentic-agent-kit/Cargo.toml`
- Create: `crates/leaven-agentic-agent-kit/AGENTS.md`
- Create: `crates/leaven-agentic-agent-kit/src/lib.rs`
- Create: `crates/leaven-agentic-agent-kit/src/codex.rs`
- Create: `crates/leaven-agentic-agent-kit/tests/codex_profile.rs`
- Modify: `Cargo.toml`
- Modify: `crates/leaven/tests/topology_contract.rs`

**Step 1: Write failing materialization tests**

Test that a repo checkout with:

```text
agent/manifest.toml
agent/system_prompt.md
agent/AGENTS.md
agent/skills/alpha/SKILL.md
```

materializes for Codex as:

```text
system prompt text returned as provider instruction input
AGENTS.md at workspace root or configured mount
.agents/skills/alpha/SKILL.md
```

Also test symlink/copy policy as an explicit enum:

```rust
pub enum AgentKitMountMode {
    Copy,
    SymlinkPreferred,
}
```

Expected: if symlink is unavailable, materializer falls back to copy and records that in the report.

**Step 2: Implement minimal Codex materializer**

The materializer should consume a checked-out repo workspace/subdir and write the Codex ABI into the run workspace. It must not call Codex or import Codex protocol types.

**Step 3: Verify**

Run:

```bash
cargo test -p leaven-agentic-agent-kit --test codex_profile
cargo test -p leaven --test topology_contract
```

Expected: PASS.

**Step 4: Commit**

```bash
jj describe -m "leaven-agentic-agent-kit: materialize AgentKit for Codex"
jj new
```

### Task 4: Add A GEPA Codex AgentKit Bridge Smoke

**Files:**
- Create: `crates/leaven-gepa-agentic-agent-kit/Cargo.toml`
- Create: `crates/leaven-gepa-agentic-agent-kit/AGENTS.md`
- Create: `crates/leaven-gepa-agentic-agent-kit/src/lib.rs`
- Create: `crates/leaven-gepa-agentic-agent-kit/src/reflector.rs`
- Create: `crates/leaven-gepa-agentic-agent-kit/tests/codex_agent_kit_reflection.rs`
- Modify: `Cargo.toml`
- Modify: `crates/leaven/tests/topology_contract.rs`

**Step 1: Write deterministic fake-runtime test**

Use a fake runtime that edits `system_prompt.md` or `skills/alpha/SKILL.md` in the materialized kit. Assert:

- parent artifact is a `GitProgramArtifact`;
- readback imports a child revision or equivalent typed `GitProgramChange`;
- `RunContext::propose` and `apply_batch` are used;
- hook declarations are ignored as scaffold, not executed;
- `system_prompt.md` is targetable separately from `AGENTS.md`.

**Step 2: Implement the bridge by composing existing Git bridge patterns**

Reuse the `leaven-gepa-agentic-git` ownership pattern. Do not add Codex flags here.

**Step 3: Verify**

Run:

```bash
cargo test -p leaven-gepa-agentic-agent-kit --test codex_agent_kit_reflection
cargo test -p leaven --test topology_contract
```

Expected: PASS.

**Step 4: Commit**

```bash
jj describe -m "leaven-gepa-agentic-agent-kit: prove repo-backed Codex kit reflection"
jj new
```

### Task 5: Add Live Codex Gate

**Files:**
- Create: `crates/leaven-gepa-agentic-agent-kit/tests/live_codex_agent_kit.rs`
- Modify: `crates/leaven-gepa-agentic-agent-kit/AGENTS.md`

**Step 1: Write ignored live test**

Pattern after the existing live Codex tests:

```rust
#[ignore = "requires local Codex auth and LEAVEN_CODEX_LIVE=1"]
```

The test must run only when:

```bash
LEAVEN_CODEX_LIVE=1 cargo test -p leaven-gepa-agentic-agent-kit --test live_codex_agent_kit -- --ignored
```

**Step 2: Make the live test meaningful**

The live test should:

- materialize a repo-backed AgentKit;
- expose `system_prompt.md`, `AGENTS.md`, and `.agents/skills`;
- ask Codex to make a constrained edit;
- read back a typed repo change;
- reject edits outside the mutable kit subtree.

**Step 3: Verify narrow and full gates**

Run:

```bash
cargo test -p leaven-gepa-agentic-agent-kit
cargo test -p leaven --test topology_contract
```

If local auth/spend is approved, also run:

```bash
LEAVEN_CODEX_LIVE=1 cargo test -p leaven-gepa-agentic-agent-kit --test live_codex_agent_kit -- --ignored
```

**Step 4: Commit**

```bash
jj describe -m "leaven-gepa-agentic-agent-kit: add live Codex kit gate"
jj new
```

### Task 6: Completion Gate

**Files:**
- Modify if needed: `docs/specs/agentic_stage_runtime.md`
- Modify if needed: `crates/*/AGENTS.md`

**Step 1: Run focused checks**

Run:

```bash
cargo test -p leaven-artifact-agent-kit
cargo test -p leaven-agentic-agent-kit
cargo test -p leaven-gepa-agentic-agent-kit
cargo test -p leaven --test topology_contract
```

Expected: PASS.

**Step 2: Run completion gate**

Run:

```bash
just check
```

Expected: PASS, or document exact unrelated blocker with command output.

**Step 3: Final commit**

```bash
jj describe -m "agent-kit: land repo-backed Codex optimization slice"
jj new
```

## Open Decisions For Implementation

- Whether `manifest.toml` unknown fields are rejected or preserved in a metadata bag.
- Whether `system_prompt.md` lowers to Codex app-server `base_instructions`, CLI stdin preamble, or both depending on runtime.
- Whether the first implementation creates a new `leaven-agentic-agent-kit` crate or keeps the Codex profile inside `leaven-agentic-git` until a second profile exists.
- Whether live Codex proof uses CLI first or app-server first. The design prefers CLI for backend-neutral execution.
