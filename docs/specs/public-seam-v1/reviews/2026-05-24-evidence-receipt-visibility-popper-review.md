# Evidence Receipt Visibility Popper Review

Date: 2026-05-24
Reviewer: Popper (`019e5b6b-afba-7441-b862-1be458f729d6`)

Scope:

- `ps1.evidence.visibility_receipts`
- Public-seam Plan Result receipt-side evidence visibility validation.

Reviewed tranche:

- `crates/leaven-public-seam/src/result.rs`
- `crates/leaven-public-seam/tests/plan_result_evidence.rs`
- `docs/specs/public-seam-v1/conformance-matrix.yaml`

Review method:

- Read-only adversarial semantic review against the locked public-seam specs,
  schemas, conformance-matrix fake-pass traps, current code, tests, and prior
  blocker reviews.
- The reviewer was explicitly instructed that rerunning the same tests was not
  sign-off.

Findings:

- Critical: none.
- Important: the new audit rejects cited source receipts whose
  `trace_refs[*].data_classes` are absent from the evidence envelope, and the
  new negative also proves the outer result value must cover newly declared
  evidence classes. This is real semantic denial, not schema-only validation.
- Important: the tranche still validates supplied Plan Result fixtures. It does
  not prove runtime or evaluator producers persist receipt-side visibility into
  results, so a fake implementation that keeps visibility as policy metadata and
  omits receipt trace facts can still evade full row closeout.
- Minor: the new acceptance test is honest as fixture/audit evidence, but it
  would overclaim if treated as full public result projection because
  `ReceiptAudit.trace_data_classes` is private validation state.

Resolution:

- Keep `ps1.evidence.visibility_receipts` pending.
- The matrix additions are acceptable partial evidence under
  `partial_contract_*`; no row should be promoted from this tranche.

Verification evidence from main rollout:

- `cargo fmt --check`
- `cargo test -p leaven-public-seam --test plan_result_evidence -- --nocapture`
- `cargo test -p leaven-public-seam --tests -- --nocapture`
- `cargo clippy -p leaven-public-seam --tests -- -D warnings`
- `cargo test -p leaven-public-seam --test contract_package -- --nocapture`
