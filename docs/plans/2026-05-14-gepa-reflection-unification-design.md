# GEPA Reflection Unification — Design

- **Date:** 2026-05-14
- **Status:** approved design, ready to implement
- **Owning crate:** `crates/leaven-gepa` (with required changes in `crates/leaven-evidence` and `crates/leaven-run`)
- **Slug:** `#4` in the public-API cleanup workstream

This document is self-contained. An implementing agent should not need any other
conversation context. Read it top to bottom, then read the three Leaven design
skills named in §10 before writing code.

---

## 1. Why this exists

GEPA reflection is the step where the optimizer looks at how a candidate
performed and asks a reflector (an LM, or an agent in a workspace) to propose an
improved edit to one part of the artifact. Today that step has four defects:

1. **The two reflection backends see different data.** The LM-backed reflector
   projects real per-case feedback records from evidence; the agent-backed
   reflector hard-codes an *empty* record list. Same optimizer step, two
   different worlds. This is a correctness bug, not a style nit.
2. **The case input is missing.** A reflection record carries the generated
   output, score, and feedback — but not the *input the artifact ran on*.
   Reference GEPA's recommended record schema leads with `Inputs`. Reflecting
   without showing the model its own input is materially weaker.
3. **The selection of "what data reflection sees" is welded into each backend.**
   It is not a swap point. To change it you must rewrite an entire reflector.
4. **The surface is non-ergonomic.** The common case (tweak the reflection
   prompt) currently requires understanding traits and generic parameters.

This design fixes all four with **zero net-new public data types** — it renames
one existing type, adds one field, deletes one type, and changes one trait
signature.

---

## 2. Current state (how it works today)

All paths below are in `crates/leaven-gepa/src` unless noted.

### 2.1 The reflector trait

`proposer.rs:65` — `GepaReflector<P, S>`:

```rust
#[allow(async_fn_in_trait)]
pub trait GepaReflector<P, S>: Send + Sync
where
    P: OptimizationProblem,
    S: EditSurface<P::Artifact>,
{
    async fn reflect_candidate(
        &mut self,
        ctx: &mut RunContext<'_, P>,
        surface: &S,
        parent: CandidateId,
        parent_assessment: AssessmentId,
        part: S::PartId,
    ) -> Result<Option<CandidateId>, OptimizerError>;
}
```

Three implementations:
- `FixedSurfaceEdit<E>` (`proposer.rs:81`) — a deterministic scaffold fixture.
- `AgentBacked<ProposerSlot<ReflectRequest>, Runtime, Bootstrap, Parser>`
  (`proposer.rs:137`) — agent-backed reflection.
- `LmBackedReflector<L, Renderer, Parser>` (`proposer.rs:221`) — LM-backed
  reflection.

### 2.2 The divergence bug — exact lines

Each implementation builds the request *inside itself*:

- **LM-backed** (`proposer.rs:239-251`):
  ```rust
  let evidence = ctx.assessment_evidence(parent_assessment)?;
  let records = evidence.reflection_records()       // <-- real per-case records
      .into_iter()
      .map(|record| record.with_source_refs([InfoRef::Assessment(parent_assessment)]));
  let selected_feedback = SelectedFeedback {
      assessment_refs: vec![parent_assessment],
      evidence_refs: Vec::new(),
      candidate_refs: vec![parent],
      records: records.collect(),                   // <-- populated
  };
  ```
- **Agent-backed** (`proposer.rs:154-159`):
  ```rust
  let selected_feedback = SelectedFeedback {
      assessment_refs: vec![parent_assessment],
      evidence_refs: Vec::new(),
      candidate_refs: vec![parent],
      records: Vec::new(),                          // <-- ALWAYS EMPTY. the bug.
  };
  ```

The agent reflector receives only candidate/assessment *ids*; the LM reflector
receives projected feedback. Nothing in the type system prevents this.

### 2.3 Current types (`reflection.rs`)

- `ReflectiveFeedbackRecord` (`reflection.rs:37`) — one feedback record:
  `{ case: Option<CaseId>, score: Option<f64>, output: Option<String>,
  feedback: String, source_refs: Vec<InfoRef> }`. **No `input` field.**
- `SelectedFeedback` (`reflection.rs:56`) —
  `{ assessment_refs: Vec<AssessmentId>, evidence_refs: Vec<InfoRef>,
  candidate_refs: Vec<CandidateId>, records: Vec<ReflectiveFeedbackRecord> }`.
- `ReflectRequest<Part = String>` (`reflection.rs:106`) —
  `{ parent: CandidateId, part: Part, part_label: String,
  selected_feedback: SelectedFeedback }`.
- `GepaReflectionEvidence` (`reflection.rs:151`) — a trait on the *evidence
  type*: `fn reflection_records(&self) -> Vec<ReflectiveFeedbackRecord>`.
  Implemented for `CasewiseEvidence<ScalarEvidence>` and
  `CasewiseEvidence<CaseAssessmentEvidence>`. Because it is a trait keyed by the
  evidence type, there is exactly one projection per evidence type — it cannot
  be swapped per run.
- `ReflectionRenderer<P, S>` (`reflection.rs:194`) + `DefaultReflectionRenderer`
  — LM presentation: `ReflectRequest` → `LmRequest`.
- `ReflectionOutputParser<P, S>` (`reflection.rs:243`) + `PlainTextEditParser`
  — LM output → `ProposalBatch`.
- `LmBackedReflectorConfig` (`reflection.rs:294`) —
  `{ sampling, output, prompt_template: Option<String> }`.

### 2.4 Agent path (`agent_stage.rs`)

`GepaReflectionBootstrap` implements `AgentStageBootstrap`. Its `plan()`
(`agent_stage.rs:58`) turns a `ReflectRequest` into an `AgentStagePlan`:
prewarmed `StageQuery::Candidate` / `StageQuery::Assessment` values, a
`StageDirective`, a `StageOutputContract`, and a `StageQueryPolicy`. It does
**not** materialize the feedback records (there are none — see §2.2). The agent
can only pull assessment metadata through queries.

### 2.5 Reference: Python GEPA

Reference clone: `/Users/darin/vendor/github.com/gepa-ai/gepa` (branch `main`).
Relevant files: `src/gepa/core/adapter.py`,
`src/gepa/proposer/reflective_mutation/reflective_mutation.py`,
`src/gepa/strategies/instruction_proposal.py`.

GEPA's reflective mutation has four separately-swappable seams:
1. `evaluate(batch, candidate, capture_traces=True)` → `EvaluationBatch`.
2. `make_reflective_dataset(candidate, eval_batch, components)` → per-component
   list of records. **This is the "what data does reflection see" seam.**
   Recommended record schema: `{Inputs, Generated Outputs, Feedback}`.
3. `propose_new_texts(candidate, reflective_dataset, components)` → new text.
   Optional; default renders the dataset and calls the reflection LM.
4. `reflection_prompt_template` (`str | dict[str,str] | None`) — prompt wording
   only, `<curr_param>`/`<side_info>` placeholders.

Two lessons we adopt:
- The proposer builds the reflective dataset **once**
  (`reflective_mutation.py:341`) and hands it down; `propose_new_texts` never
  re-derives it. Build-once-pass-down.
- The string `reflection_prompt_template` works for GEPA because GEPA is
  LM-only. We have an agent backend too, so "presentation" is not universally a
  string — it is *render to a prompt* (LM) or *materialize to a workspace*
  (agent). The string template survives only as an LM-reflector sub-knob.

We deliberately diverge from GEPA in one place: GEPA's records are untyped
`dict`s; ours are a typed struct. Typed records are the Leaven contract — the
person writing a custom dataset builder gets a real, documented shape.

---

## 3. Design decisions

### D1 — Build once, pass down (fixes the divergence structurally)

`reflect_candidate` no longer receives `parent` / `parent_assessment` / `part`
separately and builds its own request. It receives a **fully-built
`ReflectRequest`**. The GEPA optimizer loop builds that request exactly once,
via the reflective-dataset builder, then passes the same value to whichever
reflector is configured. There is no longer a place for a backend to project
data differently — the bug becomes unrepresentable.

New trait signature (`proposer.rs`):

```rust
#[allow(async_fn_in_trait)]
pub trait GepaReflector<P, S>: Send + Sync
where
    P: OptimizationProblem,
    S: EditSurface<P::Artifact>,
{
    async fn reflect_candidate(
        &mut self,
        ctx: &mut RunContext<'_, P>,
        surface: &S,
        request: ReflectRequest<S::PartId>,
    ) -> Result<Option<CandidateId>, OptimizerError>;
}
```

### D2 — Zero net-new data types

- **Rename** `ReflectiveFeedbackRecord` → `ReflectiveExample` and **add `input`**
  (see D3). Final shape:
  ```rust
  /// One evaluated case, projected for GEPA reflection.
  #[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
  pub struct ReflectiveExample {
      pub case: Option<CaseId>,
      pub input: String,            // NEW — see D3
      pub output: Option<String>,
      pub score: Option<f64>,
      pub feedback: String,
      pub source_refs: Vec<InfoRef>,
  }
  ```
- **Delete `SelectedFeedback`.** It bundled three ref vectors plus the records.
  The records become the example list; the provenance refs move onto
  `ReflectRequest` (for `informed_by` lowering) and onto each example's
  `source_refs`.
- **Keep `ReflectRequest`**, retargeted at the example list:
  ```rust
  #[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
  pub struct ReflectRequest<Part = String> {
      pub parent: CandidateId,
      pub part: Part,
      pub part_label: String,
      pub examples: Vec<ReflectiveExample>,
      /// Provenance refs lowered into the resulting proposal's `informed_by`.
      pub source_refs: Vec<InfoRef>,
  }
  ```
- **No `ReflectionTarget` type** — `ReflectRequest` already carries
  `parent`/`part`/`part_label`.
- **No `ReflectiveDataset` wrapper type** — `Vec<ReflectiveExample>` says
  everything a wrapper would, since each example carries its own `source_refs`
  and GEPA reflects on one part at a time. If multi-part reflection is added
  later, a keyed type earns itself *then*.

Net: one rename, one new field, one deletion. The advanced user who writes a
custom dataset builder constructs exactly one type — `ReflectiveExample`.

### D3 — The case input becomes first-class evaluation evidence

Slice #1 of this workstream already made the generated *output* first-class
evidence: `CaseAssessmentEvidence` (in `crates/leaven-evidence`) carries
`output: OutputRecord`. The input is just as much a fact of an evaluation as the
output, and its absence is the direct cause of defect #2.

**Decision:** add an `input: String` field to `CaseAssessmentEvidence`, captured
by the evaluator at evaluation time, exactly parallel to `output`. The
default reflective-dataset builder then reads `evidence.input()` for each case.

The evaluator (`crates/leaven-run/src/evaluator.rs`) has the case value at
evaluation time. It renders the case to a `String` for the evidence record. The
implementing agent must choose the rendering mechanism:

- **Preferred:** require `P::Case: Display` (or the nearest existing
  case-render capability) and render via that. Simple, no new public seam.
- **Fallback if `Display` is too restrictive:** a case-render closure supplied
  on the `optimize(...)` builder.

This is the one decision in this document that touches a public bound. Use the
preferred path unless an existing constraint makes it impossible; if you must
use the fallback, keep it a single optional builder method, not a new trait.

`ReflectiveExample.input` is a plain `String`. Projecting a domain case into a
reviewable view is domain work; it is done once, in the evaluator, and stored.

### D4 — The selection seam: a swappable reflective-dataset builder

"What data does reflection see" becomes one named, swappable seam. It is a
function from run state to the example list:

```rust
/// Builds the reflection examples for one parent candidate + selected part.
/// Default implementation = GEPA-parity projection (one example per evaluated
/// case: input, output, score, feedback).
#[allow(async_fn_in_trait)]
pub trait ReflectiveDatasetBuilder<P, S>: Send + Sync
where
    P: OptimizationProblem,
    S: EditSurface<P::Artifact>,
{
    async fn build(
        &self,
        ctx: &mut RunContext<'_, P>,
        parent: CandidateId,
        parent_assessment: AssessmentId,
        part: &S::PartId,
    ) -> Result<Vec<ReflectiveExample>, ReflectionError>;
}
```

Provide a blanket implementation for closures so a user can pass a plain
function instead of a named type:

```rust
impl<P, S, F, Fut> ReflectiveDatasetBuilder<P, S> for F
where
    F: Fn(&mut RunContext<'_, P>, CandidateId, AssessmentId, &S::PartId) -> Fut + Send + Sync,
    Fut: Future<Output = Result<Vec<ReflectiveExample>, ReflectionError>> + Send,
    // ... P, S bounds
{ /* ... */ }
```

The default builder reproduces today's `GepaReflectionEvidence::reflection_records`
behaviour, generalized: it looks up `ctx.assessment_evidence(parent_assessment)`,
projects one `ReflectiveExample` per case from `CaseAssessmentEvidence`
(`case`, `input`, `output`, `score`, `feedback`), and attaches
`InfoRef::Assessment(parent_assessment)` provenance.

The builder receives `&mut RunContext` (not just the latest evidence) so that a
custom builder can reach history, sibling candidates, or diffs — the agent-case
power GEPA's `eval_batch`-only signature lacks.

**`GepaReflectionEvidence` is deleted.** Its `reflection_records` logic moves
into the default builder as a private projection helper. A type-keyed trait is
the wrong shape for a per-run swappable strategy.

### D5 — Presentation is backend-specific; both backends consume the same request

The reflective-dataset builder runs once per reflection step. Its output
(`ReflectRequest`) feeds whichever reflector is configured:

- **LM reflector** — `ReflectionRenderer` renders `ReflectRequest` (now with
  `examples`, including `input`) into an `LmRequest`. Mostly unchanged from
  today; update `render_feedback_records` (`reflection.rs:335`) to emit an
  `## Input` section.
- **Agent reflector** — a presentation twin that **materializes** the
  `examples` into the agent workspace as files, in addition to the prewarmed
  queries `GepaReflectionBootstrap` already plans. The agent must receive the
  actual examples as workspace content, not only assessment-id queries.

Selection (D4) and presentation (D5) are separate seams. Do not merge them.

### D6 — Ergonomic ladder on the reflector builder

The user picks a reflector backend, and each sub-seam has a default. The common
case touches no traits.

```rust
// minimal — every seam defaulted:
Gepa::reflect_with_lm(lm, model)

// tweak the prompt wording (LM-only knob):
Gepa::reflect_with_lm(lm, model)
    .prompt_template(MY_TEMPLATE)

// swap the selection seam (closure — applies to LM and agent identically):
Gepa::reflect_with_lm(lm, model)
    .reflective_dataset(|ctx, parent, assessment, part| async move { ... })

// agent backend — same selection default, different presentation:
Gepa::reflect_with_agent(workspace_factory, runtime)
    .materialize(|examples, workspace| { ... })
```

Each seam is a builder slot accepting `impl Trait` with a closure blanket impl,
and each has a default. Only a wholly custom `GepaReflector` is a hand-written
trait impl.

### D7 — Configuration scope

There are three nested configuration scopes; keep each seam in its owning scope:
- `optimize(seed)` — data, budget, runner, scorer, execution.
- `Gepa::builder()` — surface, population, candidate/part selectors, gate,
  validation, and *which* reflector.
- the reflector (`reflect_with_lm` / `reflect_with_agent`) — lm/runtime, the
  reflective-dataset builder, renderer/materializer, parser, prompt template.

The reflective-dataset builder lives on the **reflector**, not on the `Gepa`
builder — the reflector is what consumes it.

---

## 4. The minimal user program (target)

After this change, the ordinary GEPA user writes:

```rust
optimize(AimePrompt::new(seed))
    .train(train).validation(validation).test(test)
    .runner(|prompt, case| async { /* RunOutput */ })
    .score(score_answer)
    .using(Gepa::reflect_with_lm(lm, model).surface(AimePromptSurface))
    .budget(budget)
    .run().await?
```

No reflection type from this document appears in that program. They are all in
the `extend` route of the public surface (see §9).

---

## 5. The divergence fix, concretely

1. In the GEPA optimizer loop (`optimizer.rs` — locate the existing
   `reflect_candidate` call site; per the crate `AGENTS.md` the loop is "select
   parent, evaluate train partition casewise, select a surface part, call a
   `GepaReflector`, apply the proposal batch"):
   - after part selection, call the configured `ReflectiveDatasetBuilder::build`
     once;
   - assemble a `ReflectRequest { parent, part, part_label, examples, source_refs }`;
   - call `reflector.reflect_candidate(ctx, surface, request)`.
2. In `LmBackedReflector::reflect_candidate` — delete the
   `assessment_evidence` + `reflection_records` + `SelectedFeedback` block
   (`proposer.rs:239-254`). Use `request.examples` directly via the renderer.
3. In the `AgentBacked` `reflect_candidate` impl — delete the
   `SelectedFeedback { records: Vec::new() }` block (`proposer.rs:154-161`). Use
   `request` directly; materialize `request.examples` into the workspace.
4. `FixedSurfaceEdit`'s impl takes the new signature too; it ignores `examples`.

---

## 6. Regression test for the bug (required)

Add one scenario test that would have caught the original divergence: for the
same `(parent, parent_assessment, part)`, the LM reflector and the agent
reflector must receive **byte-identical** `ReflectRequest.examples`. Place it in
`crates/leaven-gepa/tests/` (new file or extend `agent_stage_routing.rs`). This
is the durable proof that D1 holds.

---

## 7. Migration / hard cutover — file checklist

This is a hard cutover (no compatibility shims, no parallel old/new paths — per
repo `AGENTS.md`).

`crates/leaven-evidence`:
- `CaseAssessmentEvidence` — add `input: String` field, constructor parameter,
  and `input()` accessor, parallel to `output`.

`crates/leaven-run`:
- `evaluator.rs` — capture the rendered case input and pass it to
  `CaseAssessmentEvidence::new(...)` (see D3).

`crates/leaven-gepa`:
- `reflection.rs` — rename `ReflectiveFeedbackRecord` → `ReflectiveExample`,
  add `input`; delete `SelectedFeedback`; retarget `ReflectRequest`; delete
  `GepaReflectionEvidence`; add `ReflectiveDatasetBuilder` + default + closure
  blanket impl; add a `ReflectionError`; update `DefaultReflectionRenderer` /
  `render_feedback_records` to emit `## Input`.
- `proposer.rs` — new `GepaReflector` signature; update all three impls;
  delete the inline request-building blocks.
- `agent_stage.rs` — add example materialization; the agent receives
  `request.examples` as workspace files.
- `optimizer.rs` — call the dataset builder once, build the `ReflectRequest`,
  pass it to `reflect_candidate`.
- builder/`lib.rs` — `Gepa::reflect_with_lm` / `reflect_with_agent` constructors
  and the seam slots (`.prompt_template`, `.reflective_dataset`, `.render`,
  `.materialize`, `.parse`).
- `AGENTS.md` — update the reflection decision card and the "Local Bait"
  section that names `SelectedFeedback` / `ReflectiveMutation` etc.

`examples/p8_aime_gepa`:
- `aime_lm_reflector` and the reflector type alias — adopt the new
  constructors/seams.
- `AimeCase` — ensure it satisfies the case-render mechanism chosen in D3.

`crates/leaven-gepa/tests`:
- `lm_reflection.rs`, `agent_stage_routing.rs`, `gepa_smoke.rs` — update to the
  new types and signature; add the §6 regression test.

---

## 8. Error handling

Add a `ReflectionError` for the dataset-builder seam (a builder can fail —
missing evidence, projection failure). Follow the `leaven-error-design` skill:
typed enum, `thiserror`, source preservation. Reflector failures continue to
surface as `OptimizerError` / `ProposalError` as today.

---

## 9. Scope boundaries

- **Do not touch the public-surface route split.** The `crates/leaven/src/{lib,
  prelude,extend,plumbing}.rs` files and `crates/leaven/tests/public_surface_contract.rs`
  are owned by a separate workstream. The new public names from this document
  (`ReflectiveExample`, `ReflectRequest`, `ReflectiveDatasetBuilder`,
  `ReflectionError`, `Gepa::reflect_with_lm`, etc.) belong in the **`extend`**
  route — register them there only if/when that route work has landed; otherwise
  leave a note for that workstream. Ordinary users never see them.
- This change is confined to `leaven-gepa`, `leaven-evidence`, `leaven-run`,
  the `p8` example, and tests. Do not modify the engine, stores, or workspaces.
- Do not add a compatibility shim or a parallel old/new reflection path.

---

## 10. Required reading before implementing

Read these repo-local skills once, before writing code:
- `leaven-type-design` — `ReflectiveExample`, the `input` field, deleting
  `SelectedFeedback`.
- `leaven-trait-design` — the `GepaReflector` signature change, the
  `ReflectiveDatasetBuilder` seam, closure blanket impls, cold/hot boundaries.
- `leaven-error-design` — `ReflectionError`.

Also read `crates/leaven-gepa/AGENTS.md` (reflection decision cards and bait
notes) and `crates/leaven-run/AGENTS.md` (the `Score`/`RunOutput`/evidence
boundary, already updated by slice #1).

---

## 11. Verification

Iterate with:
- `cargo nextest run -p leaven-gepa` — surface lowering, reflectors, the §6
  regression test.
- `cargo nextest run -p leaven-gepa --test gepa_smoke`
- `cargo nextest run -p leaven-gepa --test agent_stage_routing`
- `cargo nextest run -p leaven-gepa --test lm_reflection`
- `cargo nextest run -p leaven-evidence -p leaven-run` — the evidence/evaluator
  changes.
- `cargo test -p leaven --test topology_contract` — if dependencies change.

Completion gate (must pass before claiming done):
- `just check` — formatting, line-count lint, clippy, SLA-enforced tests, line
  and branch coverage ratchet.
- `just milestone-p8` (deterministic, with `LEAVEN_AIME_CACHE` /
  `LEAVEN_AIME_LIVE_OPENAI` unset) — the public GEPA reflection proof.

Do not claim completion without showing the actual command output.

---

## 12. Definition of done

1. `GepaReflector::reflect_candidate` takes a pre-built `ReflectRequest`; no
   reflector builds its own request.
2. The LM reflector and the agent reflector provably receive identical
   `examples` for identical inputs (§6 test passes).
3. `ReflectiveExample` carries `input`; `CaseAssessmentEvidence` carries `input`;
   the evaluator captures it.
4. The reflective-dataset builder is a swappable seam with a GEPA-parity
   default and a closure blanket impl.
5. The agent reflector materializes the examples into the workspace.
6. `SelectedFeedback` and `GepaReflectionEvidence` are deleted; no net-new data
   types beyond the `ReflectiveExample` rename.
7. The `p8` example uses the new surface; `just check` and `just milestone-p8`
   are green.
