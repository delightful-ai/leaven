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
pub struct ReflectiveCase {
    pub case_id: Option<CaseId>,
    pub input: ReflectiveValue,
    pub expected: Option<ReflectiveValue>,
    pub runs: Vec<ReflectiveRun>,
    pub source_refs: Vec<InfoRef>,
}

impl ReflectiveCase {
    /// Flat constructor for single-attempt single-agent cases.
    pub fn from_example(
        input: ReflectiveValue,
        expected: Option<ReflectiveValue>,
        produced: Option<ReflectiveValue>,
        score: Option<f64>,
        feedback: impl Into<String>,
    ) -> Self { /* one case with one run */ }
}

pub struct ReflectiveRun {
    pub run_id: CaseRunId,
    pub agent_id: Option<AgentId>,
    pub attempt_index: Option<usize>,
    pub produced: Option<ReflectiveValue>,
    pub score: Option<f64>,
    pub max_score: Option<f64>,
    pub passed: Option<bool>,
    pub feedback: String,
    pub checks: Option<Checks>,
    /// LM-paradigm flat field rendering (paper-parity). Used by
    /// DefaultReflectionRenderer. Empty for agent-paradigm cases.
    pub side_info: Vec<(String, ReflectiveSideInfoValue)>,
    /// Agent-paradigm typed evidence. Empty for LM-only cases.
    pub attachments: Vec<Attachment>,
    pub source_refs: Vec<InfoRef>,
}

pub enum ReflectiveValue {
    Text(String),
    Json(serde_json::Value),
    File(TraceRef),
    Mapping(Vec<(String, ReflectiveValue)>),
}

pub struct Checks { pub passes: Vec<Check>, pub fails: Vec<Check> }
pub struct Check { pub id: String, pub requirement: String, pub reason: Option<String> }

// Attachment and AttachmentKind live in leaven-evidence, not leaven-gepa. The
// GEPA crate re-exports `leaven_evidence::Attachment` as
// `leaven_gepa::Attachment` for ergonomics.
pub use leaven_evidence::{Attachment, AttachmentKind};
```

This type set remains GEPA-level and benchmark-neutral. Domain-specific data
must be lowered through an explicit reflective dataset builder.

The case/run split is structural: one `ReflectiveCase` carries one case's input
+ expected; each `ReflectiveRun` inside it carries one attempt's evidence. The
default `GepaReflectiveDataset` emits one case with exactly one run
(single-agent, single-attempt, matches today's semantics). Multi-agent and
multi-attempt dataset builders are accommodated by the schema without further
changes; see `docs/specs/typed_signature_adapter_contract.md` §3.

`ReflectiveValue` replaces the old flat `input: String` /
`output: Option<String>`. It permits text, structured JSON, file refs, or
ordered mappings; the choice belongs to the dataset builder.

`Attachment` carries typed agent-paradigm evidence (transcripts, structured
JSON, text, file refs) per `docs/specs/typed_signature_adapter_contract.md` §3.
Each `AttachmentKind` has a fixed materialization rule when the workspace
runner lowers it to disk. Artifact-specific concepts, such as skill-use events
for a SkillBank, live in `Attachment::Json` with whatever shape the artifact's
evidence projection produces; they are not a new variant.

`side_info: Vec<(String, ReflectiveSideInfoValue)>` is the LM-paradigm
flat-field carrier for paper-parity rendering. When non-empty, the LM
reflector emits those fields verbatim. For agent-paradigm cases, `side_info` is
empty and evidence flows through `attachments`.

LM-paradigm side_info ordering for optimize-anything AIME records:

```text
score
input
prompt
output
reasoning
execution_feedback
```

P8 AIME should use an AIME-specific reflective dataset builder or a generic
target-safe case projection that renders `AimeInput.problem`, never
`AimeTarget.answer` or `AimeTarget.solution`.

## 4. Default Builder Rule

`GepaReflectiveDataset` may remain the default GEPA-parity builder, but it must
not teach that arbitrary `P::Case: Display` is product-safe.

Allowed default states:

1. `GepaReflectiveDataset` requires a target-safe case-input projection trait
   rather than `Display` for the whole `P::Case`; the projection produces a
   `ReflectiveValue::Text` or richer value for the `input` field of
   `ReflectiveCase`.
2. `GepaReflectiveDataset` remains a lower-level scaffold and P8 installs a
   named safe builder; or
3. Durable benchmark/product runs refuse when the selected reflective builder
   cannot prove target-safe input projection.

The hard rule: ordinary P8 must not depend on `Display` for a mixed
input/target/metadata case envelope.

The default emits exactly one `ReflectiveRun` per `ReflectiveCase`.
Multi-agent / multi-attempt datasets are produced by opt-in builders.

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

- `case_id` in `ReflectiveCase.case_id`;
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
- LM role/provider/model/sampling/output configuration;
- attachment-kind list per case;
- `ReflectionWorkspace` layout protocol version, the `kind` field of the
  workspace manifest (`reflection_workspace.v1` at v1);
- `ArtifactReflector` identity, the `signature_id` field of the workspace
  manifest from `ArtifactReflector::reflection_id()`.

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
  `ReflectiveCase`, `ReflectiveRun`, `ReflectiveValue`, `Checks`,
  renderer/parser traits, reflection provenance, and strategy fingerprint
  hooks. `ReflectiveSideInfoValue` is retained for LM-paradigm paper-parity
  rendering.
- `leaven-evidence` owns `Attachment` + `AttachmentKind`, the typed
  cross-cutting evidence vocabulary. `leaven-gepa` re-exports
  `leaven_evidence::Attachment` as `leaven_gepa::Attachment`.
- `leaven-agentic` owns `ArtifactReflector`, `ReflectionWorkspace`,
  `ReflectionLayoutConfig`, `ReadbackResult`, `ReflectionError`, and
  `ReflectionRunOutcome`; see `docs/specs/typed_signature_adapter_contract.md`.
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

- LM-backed and agent-backed reflectors see byte-identical reflective dataset
  content (`Vec<ReflectiveCase>`); their rendered task bodies differ by
  paradigm. LM uses the upstream paper template; agent uses the
  `ReflectionWorkspace` materialization. The
  `lm_and_agent_reflectors_receive_byte_identical_examples` regression asserts
  dataset-content equality, not full rendered-output equality.
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

- `cargo nextest run -p leaven-gepa --test gepa_contract lm_reflection`
- `cargo nextest run -p leaven-gepa --test gepa_smoke`
- `cargo nextest run -p p8_aime_gepa`
- `cargo test -p leaven --test topology_contract`

Completion gate remains `just check` before claiming P8 product readiness.
