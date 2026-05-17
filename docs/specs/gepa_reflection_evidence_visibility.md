# GEPA Reflection Evidence Visibility

Status: implementation spec.

This spec defines what GEPA reflection may read in P8 and in the reusable GEPA
optimizer path. It complements:

- `gepa_aime_paper_parity.md`;
- `gepa_public_private_surface.md`;
- `case_visibility_and_target_isolation.md`;
- `aime_case_report_adapter.md`;
- `resume_compatibility_fingerprints.md`.

GEPA reflection is allowed to learn from runner input, generated output, scores,
and scorer-produced feedback. It must not read raw targets, hidden answer keys,
reference solutions, or scorer-only metadata except through scorer-produced
evidence that the scoring policy deliberately makes optimizer-visible.

## 1. Current Shape

The reusable GEPA crate already has the right high-level seam:

- `ReflectiveDatasetBuilder` builds examples once;
- `ReflectRequest` carries the projected examples and provenance refs;
- LM and agent-backed reflectors consume the same `ReflectRequest`;
- proposal provenance is derived from `ReflectRequest::informed_by()`.

The current default builder, `GepaReflectiveDataset`, projects examples from
casewise evidence and then fills `ReflectiveExample.input` with:

```rust
ctx.case(case).map(ToString::to_string).unwrap_or_default()
```

That is acceptable only when `P::Case: Display` is itself a target-safe input
projection. It is not safe for product benchmark runs where the stored case
envelope contains target answers or report metadata.

## 2. Visibility Contract

Reflection examples may contain:

- `case_id`;
- runner-visible case input projection;
- candidate output text or output evidence ref;
- numeric score;
- scorer-produced feedback text;
- trace/evidence refs selected by policy;
- source/provenance refs;
- split role only when the GEPA policy is allowed to know that split.

Reflection examples must not contain by default:

- target answer;
- reference solution;
- raw `Case<I, T>` envelope;
- full metadata bag;
- hidden validation/test case data outside the configured policy;
- provider secrets or raw request credentials.

Reference solutions may influence reflection only when the scorer uses them to
produce feedback. That feedback is optimizer-visible evidence. The raw solution
remains scorer-visible target data.

## 3. Projection Types

The effective product shape is:

```rust
pub struct ReflectiveExample {
    /// Ordered upstream-style side-info fields. When non-empty, these are the
    /// model-facing reflection record.
    pub side_info: Vec<(String, String)>,
    pub case: Option<CaseId>,
    pub input: String,
    pub output: Option<String>,
    pub score: Option<f64>,
    pub feedback: String,
    pub source_refs: Vec<InfoRef>,
}
```

This type remains GEPA-level and benchmark-neutral. Domain-specific data must be
lowered through an explicit reflective dataset builder. Flat fields cover simple
case reflection; `side_info` covers upstream parity surfaces where field names
and ordering are part of the model-facing behavior, such as optimize-anything
AIME records:

```text
score
input
prompt
output
reasoning
execution_feedback
```

When `side_info` is present, the renderer emits those ordered fields directly
instead of wrapping Rust `OutputRecord` debug strings or inventing generic
headings.

P8 AIME should use an AIME-specific reflective dataset builder or a generic
target-safe case projection that renders `AimeInput.problem`, never
`AimeTarget.answer` or `AimeTarget.solution`.

## 4. Default Builder Rule

`GepaReflectiveDataset` may remain the default GEPA-parity builder, but it must
not teach that arbitrary `P::Case: Display` is product-safe.

Allowed default states:

1. `GepaReflectiveDataset` requires a target-safe case-input projection trait
   rather than `Display` for the whole `P::Case`; or
2. `GepaReflectiveDataset` remains a lower-level scaffold and P8 installs a
   named safe builder; or
3. Durable benchmark/product runs refuse when the selected reflective builder
   cannot prove target-safe input projection.

The hard rule is that ordinary P8 must not depend on `Display` for a mixed
input/target/metadata case envelope.

## 5. Evidence Boundary

Scoring is the only ordinary boundary that turns hidden target data into
optimizer-visible material.

Allowed flow:

```text
AimeTarget.answer/solution
  -> AimeScorer
  -> CaseAssessmentEvidence(score, feedback, output)
  -> ReflectiveDatasetBuilder
  -> ReflectRequest.examples
  -> GEPA reflector
```

Forbidden flow:

```text
AimeTarget.answer/solution
  -> ReflectiveDatasetBuilder
  -> ReflectRequest.examples
```

The scorer may include answer-derived feedback such as "expected 42, parsed 41"
or a natural-language solution explanation if that is the benchmark's intended
feedback policy. That policy must be part of the scorer fingerprint and report
truth.

## 6. Split Visibility

GEPA search reflection may read train/search cases selected by the batch sampler.

Validation and held-out test cases are hidden from reflection by default:

- validation may be used for selection/reporting only through an explicit
  validation policy;
- final test is report-only unless an explicit policy says otherwise;
- reflection examples must record case ids and evidence refs so audits can
  prove which split was used.

If an optimizer policy intentionally reflects on validation or test cases, that
must be an explicit product mode with report disclosure and compatibility
fingerprints. It is not the P8 default.

## 7. Source Identity

Reflection may carry source refs for audit. For AIME, the preferred projection is:

- `case_id` in `ReflectiveExample.case`;
- `InfoRef::Assessment(parent_assessment)` and candidate refs in
  `source_refs`;
- report-visible `source_id` stays in report metadata, not in the reflection
  prompt by default.

Including human-readable `source_id` in reflection prompts is allowed only if the
prompt policy declares it. If source id affects reflection content, it becomes
part of the reflector/runtime fingerprint.

## 8. Storage

Reflection artifacts must be durable:

- `ReflectRequest` or an equivalent rendered-input record is stored or
  reconstructible from graph/evidence/checkpoint state;
- LM request and response records for reflection are stored through
  provider-neutral LM evidence/cache machinery;
- proposal batches record `informed_by` refs from the reflection request;
- reports can show which assessments/cases informed a prompt edit.

If rendered prompts are too large for inline checkpoint storage, store blob refs
under the durable run store. Do not hide reflection inputs in transient logs.

## 9. Cache And Fingerprints

Reflection behavior affects resume and cache correctness. Fingerprints must cover:

- reflective dataset builder identity;
- case-input projection identity;
- evidence projection identity;
- reflection prompt template;
- output parser;
- LM role/provider/model/sampling/output configuration.

Changing any of those should refuse incompatible resume or use a distinct cache
namespace/key.

## 10. AIME Requirements

P8 AIME reflection must:

- render the AIME problem text from `AimeInput`;
- include the candidate output;
- include exact-match score;
- include scorer feedback, including reference-solution-derived explanation when
  the scorer policy emits it;
- preserve case id and assessment refs;
- avoid raw `AimeTarget` access in the reflective dataset builder;
- avoid full metadata bag projection;
- keep validation/test reflection hidden unless explicitly configured.

## 11. Implementation Routing

- `leaven-gepa` owns `ReflectiveDatasetBuilder`, `ReflectRequest`,
  `ReflectiveExample`, renderer/parser traits, reflection provenance, and
  strategy fingerprint hooks.
- `leaven-run` owns target-safe ordinary case/evidence lowering for the product
  builder surface.
- `examples/p8_aime_gepa` owns AIME-specific reflective dataset projection if no
  generic target-safe projection exists yet.
- `leaven-engine` owns graph/evidence/run-store access and must not learn
  GEPA-specific reflection semantics.

Do not put AIME target parsing in `leaven-gepa`. Do not let reflectors query the
run graph and build their own examples; the build-once `ReflectRequest` seam is
the contract.

## 12. Proof Requirements

Required tests:

- LM-backed and agent-backed reflectors still receive byte-identical examples;
- P8/AIME reflective examples include problem input, output, score, and feedback;
- P8/AIME reflective examples do not include answer, raw solution, or full
  metadata unless emitted by scorer feedback policy;
- validation/test cases are absent from reflection examples under default policy;
- proposal `informed_by` contains candidate/assessment/evidence refs from the
  selected examples;
- changing reflective dataset builder or prompt template changes runtime
  fingerprint/compatibility;
- rendered reflection inputs are stored or reconstructible from durable run
  evidence.

Focused commands:

- `cargo nextest run -p leaven-gepa --test agent_stage_routing --test lm_reflection`
- `cargo nextest run -p leaven-gepa --test gepa_smoke`
- `cargo nextest run -p p8_aime_gepa`
- `cargo test -p leaven --test topology_contract`

Completion gate remains `just check` before claiming P8 product readiness.
