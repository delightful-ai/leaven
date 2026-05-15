# Public API Cleanup — Orchestration Ledger

- **Date:** 2026-05-14
- **Status:** in progress — autonomous overnight execution
- **Purpose:** track the multi-slice public-API cleanup so it can be executed
  and verified without losing context. This is the working orchestration
  artifact; durable rules belong in specs / `AGENTS.md`.

## Context

The Leaven public API tangles ordinary user surface with internal plumbing.
This workstream separates them, enforces the separation mechanically, and fixes
the worst offenders. Work is executed as sequential slices — each is verified
with `just check` before the next begins, because every agent mutates the same
single `jj` workspace (git worktrees broke the jj setup and are not used).

## Slice status

| # | Slice | Status | Commit / artifact |
|---|-------|--------|-------------------|
| 1 | `Score`/`RunOutput`/evidence cut | DONE | `vmlwztvr` |
| 2 | Umbrella route split (prelude/extend/plumbing) | DONE | `7452d459` |
| 3 | Public-surface contract test | DONE | part of `7452d459` |
| 4 | GEPA reflection unification | IN PROGRESS | design: `docs/plans/2026-05-14-gepa-reflection-unification-design.md` |
| 5 | `OptimizationReport`/`OptimizeResult` cleanup | QUEUED | investigation in flight |
| 6 | Public-surface correctness: demotion + classification fixes | QUEUED | findings below |

Execution order: 4 → 5 → 6. Each slice: dispatch implementation agent → verify
`just check` and the slice's definition-of-done → commit → next.

## Slice 4 — GEPA reflection unification

See the dedicated design doc. Fixes: the LM-vs-agent reflection divergence bug
(`proposer.rs:158` hard-codes empty records), the missing case input in
reflection records, the non-swappable selection seam, and the non-ergonomic
surface. Zero net-new data types. Confined to `leaven-gepa`, two read-only
accessors in `leaven-engine`, `p8`, and tests.

## Slice 5 — `OptimizationReport` / `OptimizeResult` cleanup

Known offenders from the `leaven-run` audit (confirm against current code):
- `OptimizationReport.events: Vec<String>` — stringly junk drawer of event
  names; should be a typed event projection.
- `OptimizationReport.{dataset, splits}` — leak internal `Fingerprint`s.
- `OptimizeResult` — redundant public-field + accessor-method pairs.
- `default_parallelism()` (`leaven-run`) — public only for a test → `pub(crate)`.

Open design question: what `events` becomes — reuse `leaven_engine::RunEvent`
or a product-level enum. To be settled from the investigation.

## Slice 6 — Public-surface correctness (demotion + classification)

Runs LAST, against the final surface (after slices 4 and 5 settle it).

### Classification fixes (route corrections, with evidence)

- **`SurfaceError`: `plumbing` → `prelude`.** `EditSurface` is a `prelude`
  trait that ordinary users implement; `parts` and `change_part` return
  `Result<_, SurfaceError>` (`crates/leaven-surface/src/edit_surface.rs:91,111`).
  A `#[doc(hidden)]` error type on a user-implemented trait is wrong.
- **`SurfaceFingerprint`: `plumbing` → `prelude`.** `EditSurface::fingerprint`
  returns it (`edit_surface.rs:80`). Same reasoning.
- **`RunOutput`: `extend` → `prelude`.** Writing a runner is the ordinary
  `optimize().runner(...)` path. Also: its `extend.rs` doc reason still says
  "runner output, **trace**, and cost" — `trace` was deleted by slice #1. Stale.
- **`ScoreContext`: `extend` → `prelude`.** It is the scorer closure's input;
  `Score`/`ScoreError` are already `prelude`.
- **`OptimizeError`: `extend` → `prelude`.** `optimize()` returns
  `Result<_, OptimizeError>`; `OptimizeResult` is already `prelude`.
- **`ContentAddressed`: verify.** Currently `plumbing`. If `Artifact` has it as
  a supertrait and users implement `Artifact` by hand, it must be reachable →
  `prelude`. Check the `Artifact` trait definition.
- **`Proposal` / `ProposalBatch`: verify.** Currently `prelude`. If only
  proposer authors construct them, they belong in `extend`.

### Demotions (should not be public at all)

From the cold-core audit:
- `ProposalProvenance` — public `causal` / `informed_by` fields should be
  private; construction goes through builders.
- `leaven-eval` `SplitUse` / `SplitUsePolicy` / `EvaluationUse` — internal
  policy detail; `pub(crate)` or otherwise off the umbrella surface.
- The route split classified every symbol but demoted nothing; this slice does
  the actual `pub` → `pub(crate)` / field-privatization work.

Every move updates the `SURFACE` registry in
`crates/leaven/tests/public_surface_contract.rs` in the same change.

## Recovery notes

- The route-split work was originally done in a git worktree on a stale
  pre-slice-#1 base; it was re-applied and reconciled onto the current code.
  The git branch `worktree-agent-a39f6df5999a79ab9` is retained as a recovery
  point and can be deleted once the integration is trusted.
