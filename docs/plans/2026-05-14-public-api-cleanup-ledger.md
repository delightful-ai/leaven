# Public API Cleanup — Orchestration Ledger

- **Started:** 2026-05-14 — **completed (this run):** 2026-05-15 (overnight)
- **Status:** cleanup slices complete and verified; follow-up API slices decided
- **Final verification:** `just check` exit 0 (line 98.54%, branch 88.27%,
  floors intact); `just milestone-p8` exit 0 (baseline 0.0 → optimized /
  validation / held-out-test 1.0). Both run against the final commit.

## What this was

The Leaven public API tangled ordinary user surface with internal plumbing.
This workstream separated them, made the separation mechanically enforced, and
fixed the worst offenders. Each slice was implemented (mostly by a dispatched
agent), then independently verified — `just check` run by hand, diffs read,
claims checked — before the next slice began.

## Slices — all landed and verified

| # | Slice | Commits (jj change-id) |
|---|-------|------------------------|
| 1 | `Score`/`RunOutput`/evidence cut — killed `Score.structured`/`attachments`, `RunOutput.trace`, `FeedbackAttachment`; generated output became first-class evidence | `vmlwztvr` (pre-run) |
| 2/3 | Umbrella routed by audience into `prelude`/`extend`/`plumbing`; `public_surface_contract` test fails the build on any unclassified/unjustified public symbol | `zuytrmyy` |
| 4 | GEPA reflection unification — fixed the LM-vs-agent divergence bug (build-once-pass-down; `reflect_candidate` takes a pre-built `ReflectRequest`), restored the missing case `input`, made selection a swappable `ReflectiveDatasetBuilder` seam, deleted `SelectedFeedback`/`GepaReflectionEvidence` | `ontmpllm` `trwvyxrz` `ulslpyps` `pulxonrm` |
| 6 | Public-surface correctness — route reclassifications (`SurfaceError`/`SurfaceFingerprint` → prelude; `RunOutput`/`ScoreContext`/`OptimizeError` → prelude; `Proposal`/`ProposalBatch` → extend), `ProposalProvenance` lineage fields demoted to private behind accessors, dead `OptimizationReport.{dataset,splits}` fields removed | `vluqymqo` `wztwyokz` `kmlwspmx` |
| 7 | Removed the inert `WorstEvidencePart` public placeholder; `#[doc(hidden)]` on the `EditSurfacePlaceholder` internal helper | `mnmupyzt` |

Docs commits: `xzmrsxts`, `kmquwqws`, `mrmssmvn`, plus this finalization.

## What is mechanically enforced now

`crates/leaven/tests/public_surface_contract.rs` holds the `SURFACE` registry:
every umbrella-re-exported symbol is classified `prelude` / `extend` /
`plumbing`, and every `extend`/`plumbing` entry must name a concrete consumer.
A new unclassified or unjustified `pub` re-export fails `just check`. That is
the drift-stopper — slop now costs a build failure, not a code review.

## Follow-up slices decided

See `docs/plans/2026-05-15-result-facade-and-gepa-ergonomics-decisions.md`:

1. **Result facade.** Do a narrow A-lite hard cutover: rename
   `OptimizeResult<A>` to `Optimized<A>`, replace parallel best fields with
   `best: Option<BestCandidate<A>>`, rename `OptimizationReport` to concrete
   `StandardRunSummary`, and replace `events: Vec<String>` with a curated
   `RunEventSummary`. Do not add an `OptimizeResult` alias or a premature
   `Optimized<A, S>` generic. The no-best path must return a result with
   baseline report data, not an optimizer error.
2. **GEPA ergonomic constructors.** Build only `Gepa::reflect_with_lm(lm,
   model)` now as a thin defaulted entry point over
   `LmBackedReflector::with_default_renderer`. Do not build
   `reflect_with_agent(workspace, runtime)` yet; the honest agent constructor
   needs workspace factory, parser, and policy shape.

## Suggested next session

- Implement the result facade hard cutover.
- Then implement the LM-only GEPA ergonomic constructor and update one public
  GEPA example to use it.
- Run the AIME (p8) example end to end; the GEPA reflection path is now honest
  (LM and agent backends provably see identical reflective data — regression
  test `lm_and_agent_reflectors_receive_byte_identical_examples`).
- Agentic reflection is substrate-unblocked, but its public constructor is
  deliberately deferred until the factory/parser/policy shape earns an ergonomic
  surface.

## Recovery notes

- The route-split work was first done in a git worktree on a stale base, then
  re-applied. Branch `worktree-agent-a39f6df5999a79ab9` is retained as a
  recovery point — deletable now that the integration is verified.
