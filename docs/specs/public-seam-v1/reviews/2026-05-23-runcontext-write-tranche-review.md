# RunContext Write Tranche Review

Date: 2026-05-23
Reviewer: Volta (`019e56c8-8a9f-7c53-8360-33ed9ddac4d6`)
Scope: `jj diff --from stxkqtlp --to lmopwuyv`

Rows reviewed:
- `ps1.graph.runcontext_mutation_only`
- `ps1.proposal.surface_authority`
- `ps1.receipts.audit_currency`
- `ps1.replay.per_assessment`
- `ps1.evaluator.assessment_scope`

Reviewed implementation:
- `crates/leaven-run/src/public_seam/proposal_write.rs`
- `crates/leaven-run/src/public_seam/assessment_write.rs`
- `crates/leaven-run/src/public_seam.rs`
- `crates/leaven-run/src/lib.rs`
- `crates/leaven-run/tests/public_seam.rs`
- `crates/leaven-run/AGENTS.md`
- `docs/specs/public-seam-v1/conformance-matrix.yaml`

Initial findings:

1. High: proposal apply receipts could be minted from a partial forged
   `ApplyReport`. The projection checked only supplied outcomes, not exact,
   unique, complete coverage of the proposal batch.
2. Medium: assessment write `request_hash` used report order, while the
   public-seam validator hashes a sorted assessment-id set. A multi-assessment
   report could be graph-valid but rejected by the seam owner.

Resolution:

1. `PublicProposalWriteReceiptContext::proposal_apply_plan_result` now rejects
   empty apply reports, outcome count mismatches, duplicate proposal outcomes,
   failed outcomes, proposal ids outside the batch, and candidate/proposal
   mismatches against graph truth. The negative test now covers partial and
   duplicate multi-proposal apply reports.
2. `PublicAssessmentWriteReceiptContext::submit_assessments_plan_result` now
   canonicalizes assessment refs once and uses the same sorted refs for the
   value, receipt, and request hash. The positive test now reverses a
   two-assessment report before projection and validates the generated document
   through `leaven-public-seam`.

Reviewer sign-off:

No blocking findings remained after re-review. The reviewer considered these
rows safe to promote with the cited evidence:
- `ps1.graph.runcontext_mutation_only`
- `ps1.proposal.surface_authority`
- `ps1.receipts.audit_currency`
- `ps1.replay.per_assessment`
- `ps1.evaluator.assessment_scope`

Scope limit:

This sign-off covers graph-backed run-layer projection plus public-seam
validator evidence for projected Plan Result documents. It does not prove ACP
transport/session delivery, provider execution, graph mutation by the public
seam crate, or durable receipt persistence beyond the projected documents.
