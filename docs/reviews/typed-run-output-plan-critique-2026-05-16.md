# Critique: Typed Run Output Plan

Status: planning-review note. Scope is the plan `docs/plans/typed-run-output-2026-05-17.md`
against its two prompt-exports. Not a rewrite; findings only.

## 1. Top 3 under-specified seams

1. **Type-parameter collision: `O` already means *optimizer*.** `OptimizeBuilder<A, I, T, O>`
   already has four type params and the fourth (`O`) is the optimizer (`builder.rs:95`, `:109`,
   `:216`). The plan reuses `O` for *output* everywhere (Approach, Items 1-2). An implementer
   must invent the real shape — `OptimizeBuilder<A, I, T, Opt, Out>` — and the plan never says
   so. Item 2 ("OptimizeBuilder gains an output type parameter") hides the rename.
2. **`.runner(...)` must become type-state-changing.** Today `.runner` is `mut self -> Self` on
   the shared `impl<A,I,T,O>` block (`builder.rs:180-187`); `optimize()`/`train()` pre-install a
   `RunOutput::default()` (String) runner (`:80`, `:134`). For the runner to change the output
   type, `.runner` must *consume self and return a differently-parameterized builder*, and the
   default-String pre-install must be reconciled. This is the core difficulty of the "Large"
   Item 2 and is left entirely implicit ("`.runner(...)` can change the builder's output type").
3. **Renderer API + `OutputRenderError` are unowned.** Approach commits to a closure
   (`render_output: Fn(&RunOutput<O>) -> Result<OutputRecord, OutputRenderError>`, plan:122) but
   no work item names the builder method(s), and **no item defines `OutputRenderError`** — where
   it lives, and how it folds into the existing `EvaluationError` path (`evaluator.rs:243-254`).
   Item 3 assumes the error exists.

## 2. Specificity balance

- **Dropped framing.** The second export's Approach proposed a concrete
  `.render_output_with_fingerprint(fingerprint, renderer)` convenience method (export-2:80) to
  prevent fingerprint/render drift. The plan dropped it — yet that drift risk is exactly what
  Item 4 exists to guard. Re-add it as a named option in Item 3/4.
- **Missing bound.** The evaluator clones runner output into `ScoreContext`
  (`evaluator.rs:239`), so `RunOutput<O>` (hence `O`) must be `Clone`. Neither export nor plan
  states the `O: Clone` bound; an implementer hits it immediately. The `Arc<Predicted<...>>`
  escape hatch the first export gave (export-2:289) is worth keeping for non-Clone predictions.
- **Over-specification is mild.** Item 4 prescribes `RuntimeKind::OutputRenderer` and exact
  `ScoringEvaluatorIdentity` field lists — the implementation agent can see `compatibility.rs`
  and should own enum/struct naming. State the requirement (renderer identity mixes into the
  evaluator fingerprint) rather than the literal variant name.

## 3. Contradictions / missing dependencies

- **Approach contradicts Open Questions.** Approach decisively picks a renderer closure and a
  String-only `RunProblem`; Open Questions #1 and #2 then re-present both as unresolved
  (plan:382-383). A plan should not ship a recommended path *and* re-open it. Cut OQ #1-2 or
  demote them to "deferred refinements".
- **Item 6 ambiguity: mutate P8 or add a new example?** Item 6 lists `examples/p8_aime_gepa/
  src/main.rs` as a key file but also requires "existing string P8/AIME behavior continues
  unchanged." If a *new* typed example crate is intended, there is a missing work item for
  `Justfile` milestone + `topology_contract` wiring.
- **Item 7 over-coupled.** Item 7 (doc-only update to the DSRs plan) depends on Items 1-6, but
  it only needs the *API decision* (Items 1-2 design), not the full implementation.

## 4. Risk of over-planning

- **Item 5 is a no-op confirmation item** — pure "verify nothing changed," no code. Fold its
  done-when assertions into Item 6's proof and delete it. Eight items, ~6 carry real work.
- **Linear 1→2→3→4→5→6→7→8 chain over-serializes.** Items 7 and 8 are doc work and can run in
  parallel once the API lands; Item 5 disappears.
- Background (plan:8-56) is faithfully carried from the context-builder export and is good
  grounding — keep it.

## 5. Questions that would change implementation order

1. **Does `.runner(...)` return a new builder type?** If yes (it must), Item 1's "typed scorer
   test" cannot be exercised until Item 2 lands — Items 1 and 2 should merge or Item 1's typed
   test moves to Item 2.
2. **Should a typed `O` with no renderer be a compile error?** If the renderer is required at
   the type level for non-String `O`, it belongs *inside* the type-state builder (Item 2), not
   bolted on as a separate Item 3 — merging 2+3 and removing a dependency hop. Note Rust has no
   specialization, so "no ceremony for String, required for typed" must be a deliberate
   trait/impl mechanism, not an afterthought.
3. **Item 6: new example vs. P8 mutation?** The answer inserts (or not) a milestone-wiring
   work item before Item 6.
4. **Is the DSRs `Send`-future trait cutover on this plan's critical path?** The plan calls it
   a DSRs-side prerequisite; if Item 6's public-path proof is expected to include a real DSRs
   bridge test, the cutover blocks Item 6, not just Item 7.
