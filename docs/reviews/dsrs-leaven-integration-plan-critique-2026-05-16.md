# Critique: DSRs Leaven Integration Plan (2026-05-16)

**Scope:** Reviews `docs/plans/dsrs-leaven-integration-2026-05-16.md` against its source
`context_builder` export (`prompt-exports/oracle-plan-2026-05-16-145625-...`). Five
targeted axes only; no scope expansion, no rewrite.

## 1. Top 3 under-specified seams

1. **Predictor snapshot decomposition.** Item 3 says the artifact "owns ... a stable
   predictor layout, and an immutable predictor-state snapshot" in prose. The export
   gave concrete records (`DsrsPredictorSnapshot{instruction, opaque_state}`,
   `DsrsProgramState = BTreeMap<PredictorPath, ...>`, `DsrsProgramLayout`). The plan
   dropped all three. An implementer must re-derive the snapshot/state/layout split,
   and Open Question #1 means the `opaque_state` type is unknown — so both the shape
   and its contents are guesses.
2. **`DsrsModuleFactory<M>`.** Item 6 lists "a fresh-module factory" as an input and
   Item 3 says the artifact "owns or references a stable module factory," but **no work
   item creates the factory trait** and the plan never names its shape or crate home.
   The export specified the trait and resolved placement (`dsrs-leaven`, example-local
   first). Plan Open Question #3 leaves it open with no owning item.
3. **Split-hiding mechanism.** Items 5/6/8 require validation/test be hidden from
   reflection, but no item names the engine seam that enforces it. The export named
   `TrustPolicy`; the plan's own Background cites "trust/read scopes" at
   `run_context.rs:355-483` yet Item 6's "Done when" never routes to it. The implementer
   must rediscover this.

## 2. Specificity balance vs. the export

- **Over-specifies one associated type, drops framing for the rest.** Item 4 hard-fixes
  `Edit = String` but silently drops the export's companion choices (`PartId`/`Address =
  PredictorPath`, `View<'a> = &'a str`). The `String` lock is defensible (reuses the
  plain-text parser); leaving the other three unstated is the inconsistency.
- **Dropped framing — "why not `leaven-run`."** The export gave four concrete blockers
  (text-first output lowering, Leaven-native runner/scorer split, loose scaffold,
  immutable-candidate fresh-module need). The plan compresses this to a single Approach
  sentence, costing the implementer the rationale that justifies the custom-evaluator route.
- **Dropped framing — reflective dataset builder.** Item 5 requires a "custom DSRs
  reflective dataset builder" but still cites `reflection.rs:43-174,:220-340` default
  projection, leaving "reuse vs. write custom" ambiguous. The export was explicit: the
  default projection is **crate-private** and a custom builder is mandatory.
- **Report fields** (Item 6) are listed as a hard "Done when." Field selection is a
  tactical choice the implementation agent could own; "minimum fields" framing would be
  better than a fixed list.

## 3. Contradictions / missing dependencies

- **Split parity against a single-split tool.** Item 7 "Done when" requires "final split
  scores match exactly" between native `dsrs-gepa` and the bridge — but the export notes
  `dsrs-gepa::Optimizer::compile(&mut M, trainset, metric)` has **no validation/test
  split**. Validation/test parity is undefined against a train-only optimizer. This must
  be reconciled before Item 7 is implementable.
- **Factory has no creating item** (see Seam 2): Items 3/5/6 depend on it; no item owns it.
- **Item 10 is mis-ordered.** It is numbered last, but its dependency note says "Items
  3–6 reveal whether a Leaven seam gap exists." If a gap appears mid-3–6, the Leaven
  change must land *before* 5/6 complete. Item 10 is a contingency that interleaves, not
  a terminal step.
- **Item 2 removes `unimplemented!()`** as a hard cutover, but artifact/surface/evaluator
  behavior isn't implemented until Items 3–5. The export resolved this ("compile with
  placeholder orchestration"); the plan leaves the intermediate compile state unstated.

## 4. Risk of over-planning

- **Item 1** ("Reconcile against landed PR #87") is a preflight checklist, not an
  implementation item; it triple-states the "verify after merge" theme already carried by
  the Open Questions section and the Background's PR#87 caveats.
- **Item 10** has "Size: Unknown; default is zero" — a work item whose default action is
  *nothing*. This is a policy, better folded into Approach/Non-goals than given a
  numbered item with Done-when/Key-files/Dependencies.
- **Maturity-labeling is spread across Items 7, 8, and 9** ("distinguish deterministic
  from live"). Consolidate into one place.
- The **Background's PR#87 file:line citations** are explicitly volatile (Item 1
  re-verifies them all). Heavy precise citation of draft code is provisional weight.

## 5. Questions that would change implementation order

1. **Is the predictor dump/load state `Clone`/serializable?** If not, the immutable-artifact
   model in Item 3 may not hold, reshaping Items 2–3.
2. **Does `dsrs-evaluate` expose a per-example metric call, or batch-only?** If batch-only,
   a DSRs-side change is a prerequisite *before* Item 5 — a new upstream dependency the
   plan does not currently sequence.
3. **Does native `dsrs-gepa` support validation/test splits?** If not (see §3), Item 7's
   parity definition and possibly the fixture design must change, reshaping Items 6–7.
4. **Deterministic reflector fixture vs. deterministic LM adapter?** (Open Question #4) If
   an adapter is needed, it is a prerequisite sub-task before Item 6/7's deterministic lane.
5. **Is GEPA's default `ReflectiveDatasetBuilder` projection reusable for
   `CasewiseEvidence<DsrsCaseAssessmentEvidence>`?** Reusable → Item 5 shrinks;
   crate-private → custom builder is mandatory and Item 10's trigger probability rises.
