# Public Seam V1 Visibility Trace And Stage Provenance Partial Review

Date: 2026-05-24
Reviewer: Pascal (`019e5830-5229-7d91-8f6b-aaf2d72222b3`)
Scope:
- `ps1.visibility.data_class_propagation`
- `ps1.evidence.visibility_receipts`
- `ps1.stage.payload_receipts`

Rows remain pending. This review signs off only on the tranche as honest
partial public-seam validation evidence.

## Reviewed Changes

- `EvidenceEnvelopeDocument` collects public and envelope-level trace-ref data
  classes and requires target-derived top-level `data_classes` to cover them.
- Plan-result value visibility validation includes nested `Score.output`
  `blob_ref` data classes, `Score.output.trace_refs` data classes, and nested
  evidence trace-ref data classes.
- Stage payload validation rejects empty nested diagnosis `source_refs` when
  the optional field is present, and rejects scorer `target_handle` values that
  point at a different case.
- The active `reflect_then_propose` example carries nested diagnosis
  `source_refs`, but the validator does not make those schema-optional fields
  mandatory.
- Matrix evidence was updated without changing row status.
- `crates/leaven-public-seam/AGENTS.md` scopes the new behavior as advanced
  public seam validation, not runtime production proof.

## Initial Findings

1. Top-level evidence `trace_refs` bypassed data-class coverage because only
   `public.trace_refs` were collected.
2. `Score.output.blob_ref.data_classes` was collected by implementation but had
   no negative test.
3. Stage payload semantic checks were stricter than the locked schema: nested
   diagnosis `source_refs` and judge `case`/`rubric` were made mandatory even
   though the schema leaves them optional.

## Resolutions

1. `EvidenceEnvelopeDocument` now collects both `public.trace_refs` and
   envelope-level `trace_refs`; tests cover both standalone evidence and nested
   plan-result propagation.
2. `plan_result_rejects_nested_score_blob_ref_data_class_gaps` rejects an
   enclosing result value that omits a nested `Score.output.blob_ref` data
   class.
3. The stricter-than-schema stage checks were backed out. Optional nested
   diagnosis `source_refs` are checked only when present. Judge context still
   validates non-empty outputs and capability fingerprint, matching the current
   locked schema.

## Follow-Up Sign-Off

The reviewer found no blocking issues after the fixes:

- Prior finding 1 resolved by collecting both public and envelope-level trace
  refs in `evidence.rs`.
- Prior finding 2 resolved by the blob-ref negative in
  `plan_result_evidence.rs`.
- Prior finding 3 resolved by preserving locked-schema optionality in
  `stage_payload.rs`.
- Matrix/status remains honest because affected rows still say `pending`.
- `AGENTS.md` does not overclaim runtime/product proof.
- No topology leaks, MCP/watch runtime creep, provider behavior, or graph
  mutation entered `leaven-public-seam`.

Residual limits:
- This is not end-to-end runtime data-class propagation.
- This is not evidence production or receipt persistence.
- This is not proof that every runtime stage producer emits these payloads.
