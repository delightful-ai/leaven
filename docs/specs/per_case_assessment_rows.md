# Per-Case Assessment Rows And GEPA Casewise Normalization

Status: implementation spec.

Owner surfaces:

- `leaven-core`: `AssessmentGranularity`, `AssessmentTarget`, and
  `Assessment` vocabulary.
- `leaven-engine`: recording, viewing, caching, and retrieving assessment rows.
- `leaven-run`: public runner/scorer lowering and report aggregation.
- `leaven-gepa`: GEPA screening normalization, reflection examples, and
  population observation.
- `leaven-agentic`: agentic case evaluators that already run one durable unit per
  case.

This spec hard-cuts the meaning of `AssessmentGranularity::PerCase`.
`PerCase` is not "one graph assessment whose evidence happens to contain a
casewise map." It is one graph assessment row per evaluated case target.

## Problem

The current generic contract says `PerCase` means one assessment per case, but
some examples and implementation paths used a bundled shape:

```text
AssessmentTarget::EvaluationSet(set)
evidence: CasewiseEvidence { outcomes: [...] }
```

That shape made GEPA work only when the evaluator returned exactly one bundled
assessment. It also let code read `report.assessment_ids[0]` as if it were the
whole minibatch. That is not graph truth once evaluator, agentic, report, cache,
or resume code needs durable per-case records.

The hard-cut shape is:

```text
AssessmentTarget::Case { set, case }
evidence: evidence for that one case target
```

GEPA and reports may aggregate those rows into `CasewiseEvidence`, but that
aggregation is a consumer view, not the persisted evaluator output for
`PerCase`.

## Vocabulary

`Candidate` is a graph artifact version under optimization. Ordinary library
users should not have to construct candidate IDs directly when using
`optimize(...).run()`, but optimizer internals and reports cite candidates
because proposals, evaluations, and lineage are graph records.

`Case` is one dataset unit with a durable `CaseId`, runner-visible input,
optional scorer-visible target, and optional metadata governed by
`docs/specs/case_visibility_and_target_isolation.md`. `CaseId` is a Leaven
identity. External source IDs are import/report provenance and may be stored in
case metadata, but they are not runner-visible input unless the caller
deliberately includes them in the input type.

`Assessment` is one evaluator-produced graph row. Its target says what was
assessed. Its evidence type is owned by the problem domain and describes the
target of that row.

`CasewiseEvidence<E>` is a structured view over many case outcomes. GEPA
population code and reports can use it as an aggregation type. It is not the
canonical persisted row shape for `AssessmentGranularity::PerCase`.

## Granularity Law

For `EvaluationRequest::Independent { candidates, set, granularity, .. }`:

- `Aggregate` returns one assessment per candidate, targeted at
  `AssessmentTarget::EvaluationSet(resolved_set_id)`.
- `PerCase` returns one assessment per `(candidate, case)` in the resolved set,
  targeted at `AssessmentTarget::Case { set: resolved_set_id, case }`.
- `Both` returns both the aggregate rows and the per-case rows. It must not
  silently degrade to only one shape.

For `Pairwise` and `Listwise`, the same target law applies:

- aggregate judgments target the resolved evaluation set;
- per-case judgments target the evaluated case;
- the assessment variant still records the evaluated candidate relation.

If an evaluator cannot produce every row requested by the granularity, it
returns `EvaluationError::UnsupportedGranularity` or a more specific typed
evaluation error. It must not return a bundled assessment as a fallback for
`PerCase`.

## Row Invariants

Each per-case row must satisfy all of these:

1. The `AssessmentTarget` is `Case { set, case }`.
2. The `case` belongs to the resolved evaluation set for the request.
3. For independent evaluation, the row's `candidate` is one of the requested
   candidates.
4. The row evidence describes only that row's case target.
5. The row metadata may cite provenance such as source IDs or case-run record
   IDs, but metadata is not runner-visible input.
6. Row order is deterministic: requested candidate order, then resolved case
   order, unless a narrower evaluator spec defines a stronger deterministic
   order.

Cost must not be double-counted. A per-case row should carry the cost
attributable to that case. The returned `Metered<Vec<Assessment<_>>>` cost is the
sum of costful work for the report, including any shared setup that is not
assigned to a row. If `Both` includes aggregate rows derived from per-case rows,
those aggregate rows carry zero additional cost unless separate aggregate work
was actually performed.

## Engine And Cache Semantics

The graph records all assessment rows returned by the evaluator. The
`EvaluationCompleted` report stores all row IDs. Engine consumers must treat the
whole `assessment_ids` vector as the evaluation result.

Evaluation cache entries for `PerCase` requests must preserve the exact row set.
For reusable case-level caches, the compatibility key includes at least:

- evaluator identity and fingerprint;
- request shape and purpose;
- candidate identity or candidate content identity, depending on cache scope;
- resolved set identity;
- case identity;
- runner/scorer/runtime fingerprints for `leaven-run` evaluators;
- visibility policy version for target and metadata projection.

A cache hit restores rows as rows. It must not synthesize a bundled assessment.

## `leaven-run` Lowering

The public `leaven-run` scorer evaluator emits `CaseAssessmentEvidence` rows for
`PerCase`:

```text
Assessment::Independent {
  candidate,
  target: AssessmentTarget::Case { set, case },
  evidence: CaseAssessmentEvidence { score, output, feedback },
  ...
}
```

`RunProblem::Evidence` for this path is the one-case evidence type. Public
reports group rows by `(request, partition, candidate)` and present a casewise
summary. The report summary must retain all source assessment IDs, not a single
representative ID.

The runner still receives only the target-safe `RunCase<I>` input view. The
scorer receives `ScoreContext<A, I, T>` with the optional target according to the
case visibility spec. Report source IDs come from metadata/report projection,
not from runner-visible input unless the user's input type intentionally carries
them.

## Agentic Evaluator Law

Agentic task evaluators have the same contract. One attempted case run produces
one durable case run record and one case-targeted assessment row. Session
transcripts, workspace records, or scorer details may be evidence or metadata on
that row. GEPA and reports aggregate rows after recording; they do not require
agentic evaluators to create a bundled map.

## GEPA Normalization

GEPA requests `AssessmentGranularity::PerCase` for screening, feedback, and
validation when it needs instance-wise behavior. The evaluation result is the
complete `assessment_ids` vector.

GEPA must:

1. reject an empty row set;
2. inspect every returned row;
3. require the expected candidate relation for the evaluation request;
4. require `AssessmentTarget::Case { .. }` for every row used as case feedback;
5. retrieve each row's evidence;
6. project each row into a comparable scalar via a GEPA-owned evidence trait;
7. build a normalized `CasewiseEvidence<ScalarEvidence>` view for frontier and
   gate decisions;
8. preserve every source assessment ID in candidate history, reflection
   provenance, and report-facing GEPA state.

GEPA must not read only `assessment_ids[0]` for a per-case evaluation. A single
assessment is valid only when the resolved set has one case, and it is still a
case row, not a bundle.

The default reflection dataset is built from the same per-case rows. Each
`ReflectiveExample` carries:

- the row's `CaseId`;
- target-safe rendered input from the installed case set;
- generated output if the row evidence exposes it;
- comparable scalar score if the row evidence exposes it;
- feedback text if the row evidence exposes it;
- `InfoRef::Assessment(row_id)` for the row.

The reflection request-level provenance includes `InfoRef::Candidate(parent)`
plus every assessment row read while building the selected feedback. The
resulting proposal's `informed_by` is the union of request-level refs and
example-level refs.

Population code observes the normalized casewise view. A population
implementation may ignore assessment IDs if it does not persist source refs, but
it must not invent a fake aggregate assessment. Populations that expose "best
assessment" for casewise observations must store the source row IDs or rename
the API to make the aggregation explicit.

## Reports

Report facades can present casewise summaries, but they must cite all graph rows
used for the summary.

```rust
pub struct CandidateEvaluationSummary {
    pub candidate: CandidateId,
    pub request: EvaluationRequestId,
    pub assessments: Vec<AssessmentId>,
    pub average_score: Option<f64>,
    pub cases: Vec<ReportScore>,
}
```

The summary is grouped by request and candidate. Case rows that do not have
`CaseAssessmentEvidence` or another report-projectable evidence type are still
valid graph truth, but a report projector must either return a typed
unsupported-evidence error or omit the display-only score while keeping the row
ID visible.

## Test Requirements

The hard cut is complete only when these claims are tested:

- `leaven-run` `ScoringEvaluator` returns one case-targeted assessment per
  requested candidate/case for `PerCase`.
- `leaven-run` report aggregation groups multiple case rows into one candidate
  summary and retains every row ID.
- GEPA screening over a two-case minibatch consumes both assessment rows, builds
  a two-case scalar view, and never depends on `assessment_ids[0]`.
- GEPA default reflection examples carry per-row assessment provenance and do
  not expose hidden targets.
- P8 AIME reflection dataset assembly works from row IDs and preserves source
  IDs through metadata/report projection.
- Agentic evaluator tests continue to prove one durable case-run record maps to
  one case-targeted assessment row.

Focused verification should run the owning crate tests first:

```text
cargo nextest run -p leaven-run scoring_evaluator
cargo nextest run -p leaven-run optimize_builder
cargo nextest run -p leaven-gepa
```

Before claiming P8 product readiness, run the P8 milestone command and
`just check`.

