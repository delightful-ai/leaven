# Public Seam V1 Assessment Evidence Galileo Review

Date: 2026-05-24
Reviewer: Galileo (`019e5a7b-7c76-7d83-8523-684d1e15cd6a`)

Scope:
- `ps1.evidence.visibility_receipts`
- `ps1.evaluator.score_output`
- touched stage/data-visibility evidence from the same tranche

Reviewed tranche:
- `vmqksnvn` / `9e48969c`: semantic validation for embedded
  `submit_assessments` evidence, Plan Result `assessment_summary` output and
  evidence checks, and the stage object-form `CandidateRef.run` test update.

Review method:
- Read-only adversarial semantic review against the locked public-seam specs,
  schemas, conformance matrix fake-pass traps, code, tests, and AGENTS boundary
  docs.
- The reviewer was explicitly instructed not to treat rerunning the same tests
  as sign-off.

Initial findings:
- Critical: none.
- Important: `submit_assessments` evidence receipt validation invented stricter
  semantics than the locked Plan IR schema by requiring assessment-level
  duplicate read/effect receipt declarations and rejecting evidence write
  source receipts.
- Important: Plan Result `assessment_summary` validation remains partial
  evidence only. Because an `assessment_summary` row carries an assessment ref,
  optional score, and optional evidence but not the full assessed candidate
  context, it cannot prove the output is the actual assessed output by itself.
- Important: Plan Result `assessment_summary` validation over-narrowed valid
  `OutputRecord.value` shapes by accepting string values but rejecting
  structured or numeric JSON values.
- Minor: the stage object-form candidate-ref test update is consistent with
  the prior Sartre fix preserving optional `CandidateRef.run`.

Resolution:
- Removed the duplicate assessment-level receipt declaration rule and the
  invented no-write-source-receipt rule from Plan IR assessment evidence
  validation. Embedded `EvidenceEnvelope` source receipts remain the locked
  receipt carrier for this shape.
- Added positive coverage proving `submit_assessments` accepts evidence source
  receipts without duplicate assessment-level declarations, including evidence
  write source receipts.
- Preserved semantic evidence validation for embedded assessment evidence and
  kept wrong source-receipt-family rejection through `EvidenceEnvelope`.
- Broadened Plan Result `assessment_summary` output validation to accept
  structured and numeric `OutputRecord.value` content, while still rejecting
  missing score/output, missing evidence, unreceipted evidence, and missing
  assessed-output data classes.
- Kept `ps1.evaluator.score_output` and `ps1.evidence.visibility_receipts`
  pending because output identity and runtime evaluator/evidence production are
  not proven by this seam-local validation.

Follow-up sign-off:
- Critical: none.
- Important: the invented Plan IR receipt declaration semantics are removed.
- Important: the result-side output-identity limitation is recorded as a
  non-closeout reason and blocks row promotion.
- Important: structured and numeric `OutputRecord.value` shapes are accepted.
- Minor: the stage object-form candidate-ref expectation remains consistent
  with `CandidateRef.run` preservation.

Non-closeout notes:
- No matrix row is promoted by this review.
- This review records partial pending-row evidence only.
- This does not prove runtime evaluator production, actual assessed-output
  identity for every assessment summary, redaction execution, receipt
  persistence, ACP delivery, or full data-class propagation across runtime
  query/call/write execution.
