# P8 Run Report And Operator UX

Status: implementation spec.

This spec defines the ordinary operator-facing output for P8 AIME GEPA runs. It
builds on:

- `durable_runs_and_resume.md`;
- `default_cache_storage.md`;
- `resume_compatibility_fingerprints.md`;
- `aime_case_report_adapter.md`;
- `gepa_reflection_evidence_visibility.md`;
- `gepa_aime_paper_parity.md`.

P8 is not "real" until a user can inspect one run directory/result/report and
answer what ran, where it is stored, whether it can resume, what it cost, what
was cached, what data rows were evaluated, and what remains non-parity.

## 1. Product Rule

The ordinary path is:

```rust
let result = leaven::optimize(AimePrompt::seed())
    .train(dataset.train)
    .validation(dataset.validation)
    .test(dataset.test)
    .runner(AimeSolver::openai(...))
    .score(AimeScorer::exact_integer())
    .using(Gepa::aime_defaults(...))
    .budget(Budget::metric_calls(500))
    .run()
    .await?;
```

The returned `Optimized<AimePrompt>` and durable report files must expose the
same facts. CLI/example `report_lines(...)` is a projection of the public result,
not a private side channel.

## 2. Required Result Fields

The public result/report must expose:

- run id;
- run directory when local durable storage is used;
- latest checkpoint id;
- resumability;
- stop reason;
- search budget cap and spent cost;
- final-report budget cap/policy and spent cost;
- total visible cost;
- baseline train score;
- optimized train score;
- baseline validation score when present;
- optimized validation score when present;
- baseline held-out test score when present;
- optimized held-out test score when present;
- best candidate id and best prompt/artifact;
- AIME candidate table rows with candidate id, GEPA index, lineage,
  validation score/subscores, and system prompt text when the prompt can be
  reconstructed from the seed or an accepted reflection proposal;
- GEPA proposal-attempt rows that distinguish train-screen acceptance from
  admitted candidate index after full validation;
- cache mode and cache summaries;
- compatibility manifest summary;
- case-level rows with case id, source id, split, candidate, score state,
  output/feedback/trace refs;
- role/runtime summary for solver and reflection;
- provider-failure summary by role, with process-local counters separated from
  durable run-directory counters when a run resumes after a failed provider
  process;
- GEPA parity deltas and proof classification.

Absent scores must remain absent. A missing validation/test score is not `0.0`.

## 3. Storage Report

For durable runs, `RunStorage::Stored` must say:

- `resumable = true` only when a checkpoint and required compatibility manifest
  exist;
- run dir path if Leaven owns a local directory;
- latest checkpoint id;
- cache file/table locations or backend names;
- summary/report file location.

For ephemeral runs, the report must say non-resumable and cache non-durable.

If a run is stored but not resumable, the report must explain why, for example
missing checkpoint, missing runtime fingerprint, corrupt manifest, or explicit
ephemeral store.

## 4. Cache Report

The cache report is one summary, not separate user knobs.

Required structure:

```rust
pub struct RunCacheSummary {
    pub mode: CacheModeSummary,
    pub evaluation: EvaluationCacheSummary,
    pub lm_roles: Vec<LmRoleCacheSummary>,
}

pub struct EvaluationCacheSummary {
    pub durable: bool,
    pub backend: String,
    pub hits: u64,
    pub misses: u64,
    pub bypasses: Vec<CacheBypassCount>,
    pub write_errors: u64,
    pub hit_cost_zero: bool,
}

pub struct LmRoleCacheSummary {
    pub role: String,
    pub provider: String,
    pub model: String,
    pub durable: bool,
    pub backend: String,
    pub hits: u64,
    pub misses: u64,
    pub bypasses: Vec<CacheBypassCount>,
    pub write_errors: u64,
}
```

Exact names may differ. Provider-cache status must not be collapsed into
free-text metadata.

`CacheMode::Auto` is the default durable product mode. Reports should show that
the storage was available even when an evaluator bypassed cache because its
policy was `Never` or identities were missing.

## 5. Case Report Rows

Each case row should be equivalent to:

```rust
pub struct P8CaseReport {
    pub case_id: CaseId,
    pub source_id: String,
    pub split: SplitRole,
    pub candidate: CandidateId,
    pub score: ScoreState,
    pub output: ReportEvidenceRef,
    pub feedback: ReportEvidenceRef,
    pub traces: Vec<ReportEvidenceRef>,
    pub cache: Option<CacheStatusSummary>,
}
```

Default case rows must not include target answer or reference solution. Feedback
may include target-derived text only because the scorer emitted it as feedback.
Runner and scorer implementations may attach successful trace lines through
`RunOutput::with_trace` and `Score::with_trace`; reports expose those traces by
evidence ref, not by inlining raw transcripts into the summary JSON.

## 6. Proof Classification

The report must classify the run:

- deterministic mechanics/product-surface proof;
- local cached-data proof;
- live solver proof;
- live reflection proof;
- full live AIME reproduction attempt.

The report must not imply live benchmark improvement unless live provider/data
mode actually ran. Deterministic P8 output can prove the Leaven path and report
surface; it is not model-quality evidence.

## 7. Compatibility Summary

Reports must include a compatibility summary sufficient to debug resume refusals
without exposing secrets:

- dataset/case-set fingerprint;
- runner fingerprint;
- scorer fingerprint;
- evaluator fingerprint;
- optimizer fingerprint/schema summary;
- LM role fingerprints;
- cache schema/mode;
- budget policy.

Fingerprints may be redacted or shortened for human output, but the durable
manifest must store full values.

## 8. Durable Files

For the default local run dir, the target layout is:

```text
.leaven/runs/<run-id>/
  run.sqlite
  lm-cache.sqlite
  blobs/
  checkpoints/
  evidence/
  reports/
    summary.json
    p8-aime.json
```

`summary.json` is generic Leaven run summary. `p8-aime.json` is optional
domain-specific projection when the example has richer AIME report fields than
the generic result facade.

Reports must be deterministic JSON where practical so they can be diffed across
resume attempts. Report writers must replace JSON files atomically, so a killed
process leaves either the previous complete report or the next complete report,
not a torn JSON payload.

## 9. Implementation Routing

- `leaven-run` owns generic `Optimized`, `StandardRunSummary`, storage status,
  cache summary, compatibility summary, and durable summary writing.
- `leaven-engine` owns raw event/cache status and budget/cost events.
- `examples/p8_aime_gepa` owns AIME-specific report projection and CLI lines.
- `leaven-gepa` owns GEPA reflection/proposal provenance and strategy summaries.
- LM/provider crates own provider/model/runtime summaries.

Do not make examples scrape private engine graph internals. Do not invent a
separate P8-only storage path for facts the generic result already owns.

## 10. Proof Requirements

Required tests:

- durable `.run()` result reports stored/resumable run dir and checkpoint;
- `.ephemeral()` reports non-resumable and non-durable caches;
- cache summary counts hit/miss/bypass without extra user knobs;
- failed operator runs include the same safe profile, cache, run-dir, and proof
  classification context needed to diagnose cache-only replay or compatibility
  refusals without reconstructing environment variables;
- P8 report lines include run dir, resumability, stop reason, cache summary,
  budget split, source ids, and proof classification;
- P8 case report rows include output/feedback refs and do not include hidden
  target payloads;
- P8 case report rows include non-empty trace refs when runner/scorer traces are
  attached;
- absent validation/test scores remain absent;
- corrupt/missing manifest reports a typed non-resumable reason.
- generic `summary.json` and P8 `p8-aime.json` report writes are atomic.

Focused commands:

- `cargo nextest run -p leaven-run --test optimize_builder --test scoring_evaluator`
- `cargo nextest run -p p8_aime_gepa`
- `env -u LEAVEN_AIME_CACHE -u LEAVEN_AIME_LIVE_OPENAI -u LEAVEN_AIME_DETERMINISTIC_REFLECTION just milestone-p8`
- `cargo test -p leaven --test topology_contract`

Completion gate remains `just check`.
