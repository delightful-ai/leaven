# Public Seam V1 ACP, Evidence, and Stage Tranche Review

Date: 2026-05-24

Reviewer: Mendel (`019e5952-c29d-7010-a7d1-3e31e3a14798`)

Reviewed revset: `pxuqxxtn::ruknlpzm`

Reviewed commits:

- `mootuzvn` / `623beb9d` — `public-seam: audit ACP cancellation and failed call errors`
- `ryqtsxvy` / `858ced62` — `public-seam: enforce evidence receipt families`
- `wptkykyp` / `d31873bf` — `public-seam: bind stage payloads to parent surface context`
- `ruknlpzm` / `767ffa06` — `public-seam: enforce locked receipt ids for PlanError refs`

## Initial Blocker

The first review found one blocking issue: ACP lifecycle cancellation used invented
`acprec_*` receipt ids. The locked `ReceiptId` grammar in
`common.schema.json` does not include that family, and ACP cancellation was
validated manually rather than through full schema validation.

## Resolution

The follow-up commit replaced synthesized ACP disconnect receipts with
`valrec_acp_disconnect_*`, changed cancellation tests to use valid `valrec_*`
receipt ids, and made the shared public-seam `PlanError` receipt parser enforce
the locked `ReceiptId` grammar for both string and object-form `ReceiptRef`
values. The negative ACP lifecycle test now rejects `acprec_*`.

## Follow-Up Sign-Off

The follow-up review found no blocking findings for keeping the updated evidence
refs as honest partial and pending evidence. This is not sign-off to promote any
row to `proven`.

The reviewer specifically inspected:

- ACP lifecycle cancellation receipt/error semantics.
- Shared `PlanError` receipt parsing.
- Failed-call typed error and charge-audit linkage.
- Evidence envelope receipt-family enforcement, including object-form refs.
- Stage payload parent/source-ref, surface, part, reflection/proposal split, and
  target-safe projection checks.
- The active `reflect_then_propose` example.
- The relevant locked specs, schemas, profile, conformance matrix, and fake-pass
  descriptions.

## Non-Blocking Risks

- `plan_error.rs` mirrors the locked receipt-id grammar by hand. It matches the
  active schema now, but future schema changes must update this helper in the
  same change.
- Stage parent/source-ref matching uses JCS equality of the JSON value. That
  covers current string/string and object/object evidence, but it does not
  normalize semantically equivalent string and object `CandidateRef` forms.

## Status

Rows remain in their previous statuses. Pending rows remain pending, including:

- `ps1.acp.lifecycle_backpressure`
- `ps1.lm.contract`
- `ps1.evidence.visibility_receipts`
- `ps1.visibility.data_class_propagation`
- `ps1.visibility.reflector_target_safe`
- `ps1.stage.reflection_proposal_split`
- `ps1.stage.payload_receipts`

