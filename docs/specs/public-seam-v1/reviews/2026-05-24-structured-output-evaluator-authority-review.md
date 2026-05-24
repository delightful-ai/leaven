# Structured Output And Evaluator Authority Review

Date: 2026-05-24T04:46:00Z

Reviewer: Faraday (`019e57ff-3dc6-7f82-bd94-38f0cb4cb185`)

Scope:

- JSON-schema output execution validation for `lm_complete` and `agent_run`.
- Plan Result receipt validation for forged structured-output payloads with recomputed hashes.
- Capability-gated evaluator writes for `submit_assessments` and `request_evaluation`.
- Local maturity notes and conformance evidence for the touched public-seam rows.

Initial blockers and gaps:

- Structured-output validation compiled the inline schema and checked `parsed`, but did not bind `output.schema` to `output.schema_fingerprint`. A weaker inline schema could therefore ride under an authorized fingerprint.
- Missing inline-schema negative coverage existed for `lm_complete` but not `agent_run`.
- `request_evaluation` projected purpose into capability authorization, but only candidate denial had negative coverage.

Resolution:

- `plan_execution` now computes the public-seam `SchemaFingerprint` for inline JSON schemas and rejects mismatch with `output.schema_fingerprint` before schema compilation and before parsed-payload validation.
- The Plan Result receipt verifier reaches the same payload validator, so forged result values with recomputed hashes cannot bypass schema fingerprint or parsed-payload checks.
- LM and agent tests compute real `fp_schema_sha256_...` fingerprints from their inline schemas.
- Negative tests now cover missing inline schema and mismatched fingerprint for both LM and agent output contracts.
- Evaluator write authority tests now cover `assessment.submit` wrong request id, row-limit overrun, `evaluation.request` wrong candidate, and `evaluation.request` wrong purpose before host effects.

Follow-up verdict:

No blocking findings remain for this tranche.

Residual limits:

- Provider/runtime structured-output enforcement is not proven.
- ACP transport/runtime execution is not proven.
- Evaluator writes are authorized by this harness but not executed by it.
- Result replay is semantically covered through the shared validator, but this is not a full external replay service.
- Full schema-fingerprint semantics with annotation stripping or reference resolution must not be inferred from this tranche; it only binds the inline schema through the existing seam fingerprint primitive.

Matrix handling:

This review is acceptable as pending partial evidence only. It does not promote `ps1.lm.contract`, `ps1.agent.contract`, `ps1.visibility.data_class_propagation`, `ps1.evaluator.score_output`, or any other pending row.
