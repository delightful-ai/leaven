# Public API Cleanup — Orchestration Ledger

- **Started:** 2026-05-14 — **completed (this run):** 2026-05-15 (overnight)
- **Status:** cleanup slices complete and verified; two design decisions parked
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

## Parked for you — genuine design decisions, deliberately not autopiloted

See `docs/plans/2026-05-15-result-facade-and-gepa-ergonomics-decisions.md`:

1. **Result facade.** The planning spec `gepa_public_private_surface.md` §11
   specifies `Optimized<P, S>` with `best: Option<CandidateId>`; the code has
   `OptimizeResult`. The `best` non-optionality is a real correctness gap; the
   `Optimized`/`summary: S` reshape is a public-API design call on a core type
   from a *planning*-status spec — your decision, not an overnight rewrite.
   `OptimizationReport.events: Vec<String>` typing rides with this.
2. **GEPA ergonomic constructors.** `Gepa::reflect_with_lm` /
   `reflect_with_agent` (design doc D6) were not built — the load-bearing
   reflection correctness landed, the fluent ergonomic builder is a public-API
   shape worth your eye. Recommendation in the doc: two thin constructors, not
   the full fluent type-state chain.

## Suggested next session

- Decide the two items above.
- Run the AIME (p8) example end to end; the GEPA reflection path is now honest
  (LM and agent backends provably see identical reflective data — regression
  test `lm_and_agent_reflectors_receive_byte_identical_examples`).
- Agentic reflection is now unblocked: the agent-backed reflector consumes the
  same `ReflectRequest` as the LM path and materializes its `examples` into the
  workspace.

## Recovery notes

- The route-split work was first done in a git worktree on a stale base, then
  re-applied. Branch `worktree-agent-a39f6df5999a79ab9` is retained as a
  recovery point — deletable now that the integration is verified.
