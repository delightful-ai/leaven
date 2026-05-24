# Stage/Evidence Typed Data-Class Follow-Up Review

Date: 2026-05-24

Tranche revset:
- `yztnvtzwmwtv`

Scope:
- `ps1.visibility.data_class_propagation`
- `ps1.visibility.reflector_target_safe`
- `ps1.evidence.visibility_receipts`
- `ps1.stage.reflection_proposal_split`
- `ps1.stage.payload_receipts`

Reviewed tranche:
- Reflector requests reject empty example sets.
- Reflective examples must carry data classes.
- Reflective example data classes must cover typed nested `Score.output`
  surfaces: `OutputRecord.data_classes`, `blob_ref.data_classes`, and
  `trace_refs[].data_classes`.
- Arbitrary domain content under `OutputRecord.value` is not scanned as
  public-seam metadata.
- Propose requests reject embedded `ReflectionResult` payloads that are
  unreceipted or diagnosis-free.
- Evidence envelopes require private `payload_ref` blob data classes to be
  covered by the private projection and by top-level target-derived classes.

Initial adversarial findings:
- Important: the first implementation recursively scanned every nested
  `data_classes` key under `Score.output`, including unconstrained domain
  content under `OutputRecord.value`.
- Important: the first negative proof only covered direct
  `OutputRecord.data_classes`, not `blob_ref.data_classes` or
  `trace_refs[].data_classes`.
- Minor: broad error matching is acceptable for the current schema-valid
  fixtures, but message assertions would make future fake passes harder.

Resolution:
- The scanner was replaced with typed OutputRecord collection that only reads
  `data_classes`, `blob_ref.data_classes`, and `trace_refs[].data_classes`.
- Added blob-ref and trace-ref score-output gap negatives.
- Added a positive fixture proving arbitrary `OutputRecord.value.data_classes`
  domain content does not trigger public-seam data-class rejection.

Reviewer follow-up:
- Critical: none.
- Important: none.
- Minor: broad `InvalidStagePayload` matching remains acceptable for this
  tranche because the fixtures are schema-valid.
- Semantic sign-off was granted for this tranche as partial evidence only.
  The reviewer explicitly did not treat rerunning tests as sign-off.

Non-closeout notes:
- This review does not prove runtime stage lowering, ACP transport, provider
  calls, evaluator/evidence runtime production, or proposal graph mutation.
- The reviewed matrix rows remain pending.
