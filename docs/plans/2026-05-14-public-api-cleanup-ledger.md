# Public API Cleanup — Orchestration Ledger

- **Started:** 2026-05-14 — **last updated:** 2026-05-15 (overnight autonomous run)
- **Status:** in progress
- **Purpose:** track the multi-slice public-API cleanup. Working orchestration
  artifact; durable rules belong in specs / `AGENTS.md`.

## Context

The Leaven public API tangled ordinary user surface with internal plumbing.
This workstream separates them, enforces the separation mechanically, and fixes
the worst offenders. Each slice is verified with `just check` before the next
begins — every implementation agent mutates the single `jj` workspace, so they
run strictly sequentially.

## Slice status

| # | Slice | Status |
|---|-------|--------|
| 1 | `Score`/`RunOutput`/evidence cut | DONE — `vmlwztvr` |
| 2 | Umbrella route split (prelude/extend/plumbing) | DONE — `7452d459` |
| 3 | Public-surface contract test | DONE — part of `7452d459` |
| 4 | GEPA reflection unification | DONE + VERIFIED — see below |
| 5 | `OptimizationReport` cleanup | SPLIT — safe cuts → slice 6; facade reshape → decision doc |
| 6 | Public-surface correctness | IN PROGRESS — scope below |

## Slice 4 — GEPA reflection unification (DONE, verified)

Implemented `docs/plans/2026-05-14-gepa-reflection-unification-design.md`.
Fixed the LM-vs-agent reflection divergence bug (build-once-pass-down:
`reflect_candidate` takes a pre-built `ReflectRequest`, no reflector projects
its own data), added the missing case `input`, made the selection seam
swappable (`ReflectiveDatasetBuilder` + `GepaReflectiveDataset` default),
deleted `SelectedFeedback`/`GepaReflectionEvidence`.

Commits: `ontmpllm` (engine `CaseSet::get`/`RunContext::case` accessors),
`trwvyxrz` (the reflection unification), `ulslpyps` (AIME case `Display`),
`pulxonrm` (tests + divergence regression).

Verified independently (not the agent's self-report): `just check` exit 0
(line 98.54%, branch 88.27%, floors intact); `just milestone-p8` exit 0
(baseline 0.0 → optimized/validation/test 1.0); the divergence regression test
`lm_and_agent_reflectors_receive_byte_identical_examples` genuinely asserts
byte-identical examples across both backends.

**Deviation:** the D6 ergonomic constructors (`Gepa::reflect_with_lm` /
`reflect_with_agent`) were not built — see the decision doc. The load-bearing
correctness (divergence fix, swappable seam) is complete; the fluent builder is
deferred as a real API-design decision, not dropped.

## Slice 5 — split after reading the specs

Reading `durable_runs_and_resume.md` §10 and `gepa_public_private_surface.md`
§11 changed this slice. `events` is **spec-mandated** ("public event
summaries"), not a junk drawer to delete. And the result facade has a real
contract gap: the planning spec specifies `Optimized<P, S>` while the code has
`OptimizeResult`. That reshape is a genuine public-API decision — deferred to
`docs/plans/2026-05-15-result-facade-and-gepa-ergonomics-decisions.md`.

The unambiguous, spec-safe cuts fold into slice 6 (below). The `events` typing
and the facade reshape are in the decision doc.

## Slice 6 — Public-surface correctness (IN PROGRESS)

Four coherent, spec-grounded changes:

**A. Umbrella route reclassifications** (`crates/leaven/src/{prelude,extend,plumbing}.rs`
+ `tests/public_surface_contract.rs` `SURFACE` registry):
- `SurfaceError`: `plumbing` → `prelude` — `EditSurface` (a prelude trait users
  implement) returns `Result<_, SurfaceError>` from `parts`/`change_part`
  (`edit_surface.rs:91,111`).
- `SurfaceFingerprint`: `plumbing` → `prelude` — `EditSurface::fingerprint`
  returns it (`edit_surface.rs:80`).
- `RunOutput`: `extend` → `prelude` — runner authoring is the ordinary
  `optimize().runner(...)` path. Also fix its stale reason (mentions deleted
  `trace`).
- `ScoreContext`: `extend` → `prelude` — scorer-closure input; spec
  `gepa_public_private_surface.md:930` calls it "the public ... object".
- `OptimizeError`: `extend` → `prelude` — `optimize()` returns it;
  `OptimizeResult` is already prelude.
- `Proposal` / `ProposalBatch`: `prelude` → `extend` — **only if** the agent
  confirms implementing `OptimizationProblem` does not require naming them;
  construction sites are all proposer/optimizer crates and milestone examples,
  not the ordinary-user path.

**B. `ProposalProvenance` field demotion** (`crates/leaven-core/src/proposal.rs`):
- `ProposalProvenance.causal` / `.informed_by` → private + accessors.
  Construction stays via `ProposalProvenance::new` + the `ProposalBuilder`
  `.informed_by(...)` path. Public fields currently let a caller forge proposal
  lineage; the spec (`gepa_optimizer_surface.md:342,362`) makes lineage
  load-bearing. `ProposalProvenance` the type stays in `extend`.

**C. Delete dead `OptimizationReport.{dataset,splits}`** (`crates/leaven-run/src/result.rs`,
`builder.rs`): zero-caller duplicate `Fingerprint`s of `evaluation.{dataset,splits}`.
The canonical fingerprints in `EvaluationReport` stay; the membership-change
behavior is preserved there.

**D. `default_parallelism()` → `pub(crate)`** (`crates/leaven-run/src/evaluator.rs`):
internal helper, no external caller.

## Open decisions for the morning

`docs/plans/2026-05-15-result-facade-and-gepa-ergonomics-decisions.md`:
1. Result facade: `OptimizeResult` vs the planning-spec `Optimized<P, S>`;
   and the `OptimizationReport.events` typing.
2. GEPA ergonomic constructors (`Gepa::reflect_with_lm` / `reflect_with_agent`).

## Recovery notes

- The route-split work was originally done in a git worktree on a stale base,
  then re-applied. The branch `worktree-agent-a39f6df5999a79ab9` is retained as
  a recovery point; deletable once the integration is trusted.
