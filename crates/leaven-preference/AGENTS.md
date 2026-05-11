## Boundary
This crate owns stateless preference relations over reusable evidence values.

It may interpret evidence into `leaven-core::Preference`, but it must not own population state, fitted models, graph mutation, evaluator storage, or optimizer rhythm.

## Routing
- Put scalar ordering helpers in `src/scalar.rs`; the finite-score invariant is owned by `leaven-evidence::ScalarEvidence`.
- Put list/ranking and Pareto preference relation vocabulary in their named modules when behavior lands.
- Put Bradley-Terry, Plackett-Luce, tournament, frontier, archive, and niche state in `leaven-population`.
- Put dynamic stage traits and `DynPreferenceRelation` object behavior in `leaven-engine`; add adapters here only when they remain stateless preference implementations.

## Current Public-Maturity Split
- Behavior-bearing today: `HigherScoreIsBetter` and `LowerScoreIsBetter` over
  `ScalarEvidence` have tests and rely on evidence-level finite construction.
- Public placeholders today: `BordaPreference`, `CopelandPreference`,
  `LexicographicPreference`, and `ParetoPreference` are production-looking unit
  structs without algorithms or laws. Do not re-export them as standard behavior
  through `leaven-std` or product examples until implemented.

## Local Helper Stack
- Scalar preferences should accept `ScalarEvidence`, not raw `f64`, so the
  non-finite guard stays centralized.
- Add ranking/listwise behavior only with an evidence input shape and tie policy
  tests. A marker name alone is not a preference relation.
- Keep all fitted or observation-dependent state in `leaven-population`; this
  crate should stay reusable and stateless.

## Local Bait
- The crate depends near engine/population vocabulary, so it is tempting to put selection state here. Do not. Preference answers "which result is better"; population answers "what state survives observations."
- Ranking module names are not proof that ranking algorithms are implemented. Add tests with the behavior before expanding exports.
- A zero-sized type is acceptable only when it has real marker laws or behavior
  tests. Current ranking/Pareto names are audit-visible placeholders, not
  canonical examples.

## Proof Anchors
- `cargo nextest run -p leaven-preference` proves the scalar stateless
  preference behavior. It does not prove ranking/Pareto placeholder names.
- `cargo nextest run -p leaven-population --test tournament` proves fitted pairwise preference state belongs in population, not here.
- `cargo nextest run -p leaven-engine --test stage_trait_contracts` proves engine-level preference relation object contracts outside this crate.
