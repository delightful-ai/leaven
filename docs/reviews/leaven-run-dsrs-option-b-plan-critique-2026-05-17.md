# Critique: leaven-run DSRs Option B Plan (2026-05-17)

**Scope:** Reviews `docs/plans/leaven-run-dsrs-option-b-2026-05-17.md` against the
context_builder export `prompt-exports/oracle-plan-2026-05-17-140433-option-b-plan-e96432-c039.md`.
Critique only — no scope expansion, no rewrite. One spot-check performed on the error-type seam.

## 1. Top 3 under-specified seams

1. **Rendering-failure error type is unnamed.** Item 3 says "Rendering failure
   becomes an evaluation failure with incurred runner/scorer cost preserved" and
   names `OutputRenderError` + `OptimizeError::MissingOutputRenderer`. But
   `ScoringEvaluator::evaluate_job` returns `EvaluationError` via
   `EvaluationError::with_cost_source(...)` (`evaluator.rs:225`, `:242`). The plan
   never says how `OutputRenderError` maps into `EvaluationError`, nor what cost
   string/source it carries. The export's file-by-file section did name
   `EvaluationError`; the plan dropped it. An implementer must guess the conversion.
2. **`MissingOutputRenderer` is in the wrong error family as written.** `OptimizeError`
   (`error.rs:13`) is the builder/run-orchestration error; a *missing* renderer is a
   builder-time `.run()` check, but a renderer that *fails at runtime* is evaluator-side.
   Item 3 lumps both under one bullet. The plan should split "missing renderer →
   `OptimizeError`" from "render failed → `EvaluationError`" explicitly.
3. **Old-manifest renderer compatibility.** "Missing renderer may be interpreted as
   the built-in string renderer only if needed for compatibility; evaluator
   fingerprint versioning must still prevent unsafe cache reuse"
   (Approach → Compatibility) is hand-wavy. The plan never confirms whether an
   evaluator fingerprint *version* field exists or whether `RunCompatibilityManifest`
   (`compatibility.rs:62-152`) needs a schema migration. Item 4 leaves the deserialization
   default to the implementer.

## 2. Specificity balance

- **Dropped useful framing:** The export's **"Risks and migration"** section is gone
  entirely. Its concrete sequencing guidance — "`.runner(...)` must become type-changing…
  update tests first" — is genuinely actionable and is not recoverable from the work
  items. Recommend re-adding a 5-line risk note. The export's **"File-by-file impact"**
  per-file `Depends on` notes were also lost; mostly redundant with Key Files, but the
  `EvaluationError` reference (seam #1) went with it.
- **Mild over-specification:** Two builder methods `.render_output` /
  `.render_output_with_fingerprint` are prescribed where one method with an optional
  fingerprint would do — an API-surface call the implementation agent should own.
  `RunOutput::typed(...)` is hedged with "such as", so that one is fine.
- The plan otherwise *added* good specificity over the export (file:line anchors in
  Background); keep those.

## 3. Contradictions / missing dependencies

- **Item 1 vs. its own dependency claim.** Item 1 (`Dependencies: None`) requires "a
  focused typed-output scorer test can inspect a non-string `ctx.output.output`." A
  meaningful scorer test needs the builder/evaluator path (Items 2–3) unless the test
  constructs `ScoreContext` directly. State which, or move the bullet to Item 6.
- **Item 5 over-declares dependencies.** Item 5 ("prove report/GEPA compatible")
  declares `Dependencies: Items 1–4`, but it asserts only that report/GEPA types are
  *unchanged*; it does not need Item 4 (renderer fingerprinting). Loosen to Items 1–3.
- **Implementation Order (8 steps) does not map 1:1 to 7 Work Items.** Order steps 3 and
  4 both decompose Item 3. Two parallel ordered lists will drift; collapse one.

## 4. Over-planning risk

- **Item 5 is not a work item.** Every "Done when" bullet is "X remains unchanged" — it
  is a verification checklist, not deliverable work. Fold it into Item 6's assertions or
  the verification gate.
- **Triple restatement of the design.** "Approach" prose (Chosen API direction /
  Renderer requirement / Evaluator data flow / Compatibility), the Work Items'
  "Done when", and "Open Questions → Known decisions" all restate the same decisions.
  Cut the "Approach" prose roughly in half; it duplicates the work items.
- The two ASCII flow diagrams (current vs. corrected) appear effectively twice. Keep one pair.

## 5. Questions that would change implementation order

- **Is missing-renderer enforced at runtime or at the type level?** The plan closes this
  ("not a type-state redesign") with little justification. If the team wants compile-time
  enforcement (e.g. a `DefaultRender` trait impl'd only for `String`), Items 2 and 3
  merge and the renderer must be threaded *with* the `Out` type change, not after it.
- **Does `.runner()`'s `self`-consuming `Out → NextOut` rebuild interact with an
  already-installed renderer field?** If `.render_output` can be called before `.runner`,
  the type-state transition must carry/discard the renderer — this reorders Items 2 and 3.
- **Is P8/AIME source-affected?** Order step 8 hedges "`just milestone-p8` if P8 behavior
  changes." Because `.runner()` becomes type-changing, P8 call sites may need edits — if
  so, that is an unlisted work item and changes the closing scope.
