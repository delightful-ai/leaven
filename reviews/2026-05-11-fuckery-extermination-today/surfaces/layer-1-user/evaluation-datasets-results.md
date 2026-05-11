# Layer 1 Evaluation, Datasets, And Results

Status: active findings recorded.

This file audits train/validation/test semantics, scoring ergonomics, result
facades, and whether users can score real traces with natural-language feedback.

## Findings

### L1-005: Score and output are too thin for optimizer feedback

- severity: high
- evidence: `crates/leaven-run/src/evidence.rs:3-54`,
  `crates/leaven-run/src/evaluator.rs:103-116`,
  `docs/specs/gepa_public_private_surface.md:892-909`,
  `docs/specs/gepa_public_private_surface.md:930-985`,
  `docs/specs/gepa_public_private_surface.md:1115-1182`
- promised behavior: score functions can return scalar scores, natural
  language feedback, structured metrics, attachments, trace references, and
  metered scoring cost. `ScoreContext` should be a typed view over candidate,
  case, output, run error, trace, history, and budget.
- actual behavior: `RunOutput` is `String + Vec<String>`. `Score` is
  `f64 + String + Vec<(String, String)>`. `ScoreContext` exposes three public
  fields: artifact, case, output. Structured feedback is flattened into trace
  strings when evidence is recorded.
- why it matters: real agent transcripts, files, model-judge rationale,
  failure categories, and feedback artifacts cannot become durable optimizer
  evidence through the ordinary API.
- correction direction: implement the rich `Score` / attachment /
  `EvidenceRef` contract and make `ScoreContext` a private-field accessor view.
  Scoring failures should be errors, not sentinel scores.

### L1-006: Dataset identity is positional, not user-stable

- severity: medium
- evidence: `crates/leaven-run/src/builder.rs:321-356`,
  `crates/leaven-eval/src/dataset.rs:44`,
  `examples/p8_aime_gepa/scripts/materialize_hf_aime.py:19`,
  `docs/specs/eval_lowering_detail.md:650-669`
- promised behavior: train / validation / test lowering should preserve stable
  case IDs, reject duplicate IDs across splits, and make reports
  case-investigable.
- actual behavior: Layer 1 accepts plain vectors and generates dense
  positional case IDs. The AIME materializer drops dataset IDs before the
  Leaven run sees them.
- why it matters: reports cannot name the original case, prove split identity,
  detect duplicates by user ID, or reproduce case-level failures cleanly.
- correction direction: accept and lower `Case` / `CaseSuite` style inputs with
  stable IDs. Keep `Vec<C>` only as an explicit dense-ID convenience path.

### L1-007: Missing evidence is reported as zero

- severity: medium
- evidence: `crates/leaven-run/src/result.rs:64-71`,
  `crates/leaven-run/src/builder.rs:454-457`
- promised behavior: reports should distinguish absent evidence, failed
  evaluation, and real numeric zero.
- actual behavior: the report average returns `0.0` for an empty assessment,
  and train-score reporting uses `.unwrap_or(0.0)`.
- why it matters: a broken evaluation can look like a valid low score. This is
  exactly the kind of proxy proof the audit is meant to reject.
- correction direction: make absent/failed scores explicit in the result
  facade. Only display numeric zero when an evaluator produced numeric zero.
