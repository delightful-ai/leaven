## Boundary
This crate owns reusable evidence value shapes: scalar scores, pairwise judgments, casewise outcomes, command/trajectory records, feedback, attribution, and placeholder shapes for diff/json/list/vector/string evidence.

Evidence here is data a stage or evaluator can produce and another component can interpret. It is not a store, scorer, population, preference relation, graph event, or evaluator registry.

## Routing
- Put finite single-objective scores in `src/scalar.rs`; non-finite refusal belongs at construction so preference and population code never decides what `NaN` means.
- Put pairwise outcomes in `src/pairwise.rs`; fitted ability state and tournament updates belong in `leaven-population`.
- Put per-case evidence containers in `src/casewise.rs`; dataset split policy belongs in `leaven-eval`, and engine case-set resolution belongs in `leaven-engine`.
- Put caller-keyed attribution in `AttributableEvidence`; do not make attribution keys surface-only, path-only, or GEPA-only.
- Store references and persistence capabilities belong in `leaven-store-*`, not in evidence values.

## Current Public-Maturity Split
- Behavior-bearing today: scalar scores, pairwise judgments, casewise sparse
  containers, command/agent trajectory records, and attribution traits have
  local tests.
- Concrete but under-tested today: `ScoredFeedbackEvidence` has real fields and
  constructors but no focused test yet. Treat it as useful vocabulary, not as a
  completed reflective-feedback contract.
- Public placeholders today: `diff`, `json`, `listwise`, `mixed`,
  `score_vector`, and `string` are root-re-exported names without behavior laws.
  Do not cite them as standard evidence until they carry fields, constructors,
  and tests.
- `ScoredFeedbackEvidence` is currently the closest reusable GEPA-style feedback
  shape: scalar score, natural-language feedback, and trace lines. It is still
  evidence data, not the reflective mutation algorithm.

## Local Helper Stack
- Use `ScalarEvidence::new` for any score crossing crate boundaries; downstream
  preference/population code assumes non-finite values were refused already.
- Use `CasewiseEvidence` for sparse per-case data. Missing case IDs mean
  absence, not zero score.
- Use `OutputRecord::BlobRef` for large stdout/stderr; `OutputRecord::Inline`
  is bounded display evidence.
- Use `AttributableEvidence<K>` when evidence needs to point at surface parts,
  paths, agents, tools, modules, or user keys without making this crate know
  those key domains.

## Local Bait
- Human prose fields such as rationales and notes are debug context. Algorithms should route on typed fields such as `ScalarEvidence::score`, `PairwiseJudgment`, and `CaseOutcome`, not require prose to exist.
- Placeholder modules in `src/lib.rs` are naming reservations, not permission to hide real implementation in `lib.rs`. Move behavior into the named module first.
- The crate doc still says skeleton, and audit docs flag that as stale/ambiguous.
  Fix metadata separately from symbol maturity: some exports are real and some
  are placeholders.

## Proof Anchors
- `cargo nextest run -p leaven-evidence` proves scalar, pairwise, casewise,
  command/trajectory, and attribution behavior. It does not currently prove
  every root-re-exported evidence name.
- `cargo nextest run -p leaven-preference --test scalar` proves scalar preference callers rely on `ScalarEvidence`'s finite-score contract.
- `cargo nextest run -p leaven-population --test tournament` proves pairwise evidence feeds fitted population state outside this crate.
- Before adding an evidence name to `leaven-std`, add a focused test in this
  crate and update the public-maturity/export ledger pressure from
  `reviews/2026-05-11-fuckery-extermination-today/cross-cutting/surface-requirements.md`.
