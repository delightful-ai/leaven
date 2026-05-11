# Crate Inventory

Status: active findings recorded.

This file records the workspace crate list, intended ownership boundary, and
audit notes for crates that appear empty, stubbed, duplicated, misplaced, or
more public than their implementation maturity supports.

## Inventory

Source: `cargo metadata --no-deps` from the current workspace, plus a static
scan of `crates/*`.

### Directionally Real Core / Substrate Crates

- `leaven-kernel`
- `leaven-core`
- `leaven-surface`
- `leaven-store`
- `leaven-engine`

Audit note: these are not declared perfect; they are the crates with the most
real implementation behind their declared boundary.

### Product/User Surfaces With Public Gaps

- `leaven`
- `leaven-run`
- `leaven-eval`
- `leaven-gepa`
- `leaven-lm`
- `leaven-lm-cache`
- `leaven-lm-openai`
- `leaven-lm-mock`

Audit note: these are the highest-risk crates because users are likely to touch
them first and they currently mix real behavior with proxy examples,
placeholder names, sync-only seams, or missing runtime wiring.

### Agentic Stack, Mostly Real But Still Needs Surface Audit

- `leaven-agent`
- `leaven-agent-command`
- `leaven-agent-codex`
- `leaven-agent-codex-cli`
- `leaven-agent-codex-app-server`
- `leaven-agentic`
- `leaven-agentic-skill`
- `leaven-artifact-skill`
- `leaven-workspace`
- `leaven-workspace-local`

Audit note: the Codex app-server topology appears bounded correctly in the
current topology tests. The generic agentic surface still needs the same
public-vs-private export review as GEPA/LM.

### Mixed Real Code And Stub Exports

- `leaven-evidence`
- `leaven-population`
- `leaven-preference`
- `leaven-render`
- `leaven-std`

Audit note: these crates export standard-sounding names that are often empty
unit structs or skeletons. That is dangerous because they sit in the reusable
vocabulary layer.

### Public Placeholder / Scaffold Crates

- `leaven-artifacts`
- `leaven-artifact-git`
- `leaven-artifact-jj`
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

Audit note: these crates should either become real, stop exporting public
capability names, or move behind explicit scaffolding status. Optional feature
names should not imply usable integrations when the implementation is inert.

### Orphan / Stale Directory

- `crates/leaven-dsrs`

Audit note: the corrected topology docs mention DSRS, but this directory is not
a workspace crate: no `Cargo.toml`, no `src/lib.rs`, and no topology contract
entry. It should be deleted or hard-cut back in as a real crate.
