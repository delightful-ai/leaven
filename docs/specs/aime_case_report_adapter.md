# AIME Case And Report Adapter

Status: implementation spec.

This spec defines the P8 AIME domain adapter that lowers upstream AIME rows into
Leaven case envelopes and projects source identity back into reports. It is
subordinate to:

- `case_visibility_and_target_isolation.md`;
- `resume_compatibility_fingerprints.md`;
- `default_cache_storage.md`;
- `durable_runs_and_resume.md`;
- `gepa_aime_paper_parity.md`.

The goal is to make AIME real through Leaven's ordinary public surface without
smuggling hidden answers through runner input or report strings.

## 1. Problem

The current P8 example shape is:

```rust
struct AimeCase {
    source_id: String,
    problem: String,
    answer: i64,
    solution: String,
    needs_modular: bool,
}
```

That shape is useful as an import record, but it is not the product run case.
If it is passed directly to `.train(...)`, `.validation(...)`, or `.test(...)`,
the runner can read the answer and solution. After target isolation, AIME must
lower import records into:

- runner-visible input: problem text and any fields the candidate may use;
- scorer-visible target: answer and reference solution;
- metadata: source/provenance, split, audit, and optional report fields;
- stable `CaseId`: Leaven's typed evaluation handle.

## 2. Import Record

The raw import/cache record remains AIME-local:

```rust
pub struct AimeImportRecord {
    pub source: AimeSource,
    pub split: AimeSplit,
    pub problem: String,
    pub answer: AimeAnswer,
    pub solution: String,
    pub tags: AimeTags,
}

pub struct AimeSource {
    pub dataset: String,
    pub config: Option<String>,
    pub split: String,
    pub row_id: String,
    pub revision: Option<String>,
}

pub struct AimeAnswer {
    pub integer: i64,
    pub raw: String,
}

pub struct AimeTags {
    pub needs_modular: bool,
}
```

The materialized HuggingFace cache may keep the compact historical JSON field
`source_id`, but loading must parse or normalize it into `AimeSource`. The
canonical source id string is:

```text
dataset:config:split:row_id[@revision]
```

If a source has no config, use `default`. If a source has no explicit revision,
omit the `@revision` suffix but record the corpus cache fingerprint separately.

## 3. Run Case Types

AIME lowering produces:

```rust
pub struct AimeInput {
    pub problem: String,
}

pub struct AimeTarget {
    pub answer: AimeAnswer,
    pub solution: String,
}

pub struct AimeReportMetadata {
    pub source: AimeSource,
    pub tags: AimeTags,
}

pub type AimeRunCase = leaven_eval::Case<AimeInput, AimeTarget>;
```

Runner code receives only `RunCase<AimeInput>` or the equivalent target-free
view. It may read `case.id()` and `case.input().problem`. It must not be able to
read `AimeTarget`, `AimeReportMetadata`, source ids, split policy, tags, or
reference solution through the ordinary runner path.

Scorer code receives `ScoreContext<AimePrompt, AimeInput, AimeTarget>`. It may
read:

- case id;
- problem input;
- runner output;
- target answer;
- reference solution;
- explicitly projected scorer metadata, if any.

The default AIME exact-match scorer does not need source metadata. `source_id`
and `needs_modular` are report/stratification metadata by default, not scorer
metadata.

## 4. Stable Case Ids

`CaseId` is Leaven's stable run handle. AIME must not rely on positional ids as
the only durable identity for product runs.

Default derivation:

1. Build canonical source id bytes from `AimeSource`.
2. Hash with a domain prefix such as `leaven:aime-case:v1`.
3. Map the hash into Leaven's `CaseId` representation with collision detection.
4. If two records collide or repeat the same source id with different problem,
   target, or metadata content, fail dataset loading before run start.

The deterministic fixture may use a deterministic local source namespace such as
`deterministic:default:train:0`, but it should still lower through the same
source-id-to-case-id path so tests prove the product adapter shape.

Reports must display the source id. Humans should not have to reverse-engineer
`CaseId` values to find the upstream row.

## 5. Metadata Classes

AIME metadata is classified as:

| Field | Class | Visible To |
| --- | --- | --- |
| `source` / canonical `source_id` | provenance/report | report, audit, resume manifest |
| `split` | split/report | split policy, report |
| `needs_modular` | stratification/report | sampler/filter only if configured, report |
| `dataset revision` / corpus fingerprint | provenance/resume | report, compatibility manifest |
| scorer rubric version | scoring | scorer, evaluator fingerprint |

Putting a field in AIME metadata never makes it runner-visible.

If AIME later uses `needs_modular` for sampling, curriculum, or grouped metrics,
that use must be named in the sampler/report policy and included in the relevant
compatibility fingerprint. If the scorer reads a metadata field, that field is
no longer pure report provenance.

## 6. Dataset Lowering

The AIME loader returns:

```rust
pub struct AimeDataset {
    pub train: Vec<AimeRunCase>,
    pub validation: Vec<AimeRunCase>,
    pub test: Vec<AimeRunCase>,
    pub manifest: AimeDatasetManifest,
}

pub struct AimeDatasetManifest {
    pub corpus_fingerprint: Fingerprint,
    pub train_source: AimeSourceSet,
    pub validation_source: AimeSourceSet,
    pub test_source: AimeSourceSet,
}
```

Lowering rules:

1. Preserve train/validation/test roles from the source materializer.
2. Build `AimeInput { problem }`.
3. Build `AimeTarget { answer, solution }`.
4. Store `AimeReportMetadata` in the case metadata bag under typed or reserved
   keys owned by the AIME adapter.
5. Produce stable `CaseId`s from source identity.
6. Compute a dataset compatibility fingerprint from source ids, split roles,
   input content, target content, and scorer-visible metadata.
7. Compute a report provenance fingerprint from source ids, corpus revision,
   tags, and source manifest details.

Compatibility fingerprint and report provenance fingerprint may share bytes, but
they are conceptually distinct. Report provenance changes that do not affect
runner/scorer/evaluator behavior can be recorded without invalidating evaluation
cache entries; source or target changes that affect case identity must refuse
resume.

## 7. Runner And Solver Output

The AIME runner takes the candidate prompt plus `AimeInput`.

Required output projection:

```rust
pub struct AimeRunOutput {
    pub answer_text: String,
    pub raw_output: String,
    pub trace: Vec<AimeTraceEvent>,
    pub cost: Cost,
}
```

If backed by a language model, the runner must preserve provider-neutral LM
request/response evidence either in `AimeRunOutput`, run evidence, or trace
attachments. The optimized artifact remains `AimePrompt`; the solver wrapper and
answer parser are runtime/scorer configuration and participate in runtime
fingerprints.

## 8. Scoring

The default AIME scorer:

1. Parses an integer answer from `AimeRunOutput.answer_text` or `raw_output`.
2. Compares it to `AimeTarget.answer.integer`.
3. Emits score `1.0` for exact match and `0.0` for mismatch or parse failure.
4. Emits feedback containing the parsed answer, expected answer, and reference
   solution when available.
5. Records parse failures distinctly from wrong numeric answers.

Reference solution text can become optimizer-visible only through scorer
feedback/evidence. Raw `AimeTarget.solution` must not be passed into GEPA
reflection directly.

## 9. Report Projection

The durable P8 report must be able to answer:

```rust
pub struct AimeCaseReport {
    pub case_id: CaseId,
    pub source_id: String,
    pub source: AimeSource,
    pub split: SplitRole,
    pub candidate: CandidateId,
    pub score: ScoreState,
    pub output_ref: Option<EvidenceRef>,
    pub feedback_ref: Option<EvidenceRef>,
    pub trace_refs: Vec<EvidenceRef>,
}
```

A report case row may include generated output and feedback summaries, but it
must not include target answer or reference solution unless an explicit report
policy opts into answer disclosure. Benchmark-facing default reports should hide
targets and reference solutions.

The report must distinguish:

- score absent because the split was not evaluated;
- score absent because evidence is missing;
- scorer/runtime error;
- present score `0.0`.

Absent scores must never be normalized to `0.0`.

## 10. Resume And Cache Obligations

AIME contributes to durable compatibility:

- source ids and split membership;
- problem text fingerprint;
- answer and solution fingerprint;
- scorer-visible metadata fingerprint;
- corpus/cache manifest fingerprint;
- solver runtime fingerprint;
- scorer parser/rubric fingerprint.

Evaluation cache keys may continue to use engine-level
`EvaluationCacheKey`, but `case_set_version` and evaluator fingerprints must be
derived from AIME case compatibility rather than from positional case count.

LM response cache entries are keyed by provider-neutral LM request and role
fingerprint. AIME source ids are not LM cache keys unless the prompt actually
contains them.

## 11. Implementation Routing

- `examples/p8_aime_gepa` owns AIME import records, materializer/cache JSON,
  deterministic fixture cases, runner/scorer wiring, and P8 report projection.
- `leaven-eval` owns generic case envelope and dataset primitives.
- `leaven-run` owns generic report facades and product builder lowering.
- `leaven-gepa` owns reflection selection and consumes scorer-produced evidence,
  not raw AIME targets.
- `leaven-lm-*` owns provider execution and request/response fingerprints.

Do not introduce a reusable AIME crate until another benchmark needs the same
types. Do not move AIME source parsing into `leaven-engine` or `leaven-gepa`.

## 12. Required Tests

P8 adapter tests:

- deterministic fixture lowers to `Case<AimeInput, AimeTarget>` with stable
  source-derived `CaseId`s;
- runner receives problem input but not target or source metadata;
- scorer receives target answer and reference solution;
- report rows contain `case_id`, `source_id`, split, output/feedback refs, and
  no hidden target payload by default;
- cache JSON loading preserves train/validation/test roles and source ids;
- duplicate source id with different content refuses during loading;
- changed answer or problem changes dataset compatibility fingerprint;
- report-only metadata change does not affect scorer fingerprint unless
  projected into scorer metadata.

Focused commands:

- `cargo nextest run -p p8_aime_gepa`
- `env -u LEAVEN_AIME_CACHE -u LEAVEN_AIME_LIVE_OPENAI -u LEAVEN_AIME_LIVE_OPENAI_REFLECTION just milestone-p8`
- `cargo test -p leaven --test topology_contract`

Completion gate remains `just check` before claiming P8 product readiness.
