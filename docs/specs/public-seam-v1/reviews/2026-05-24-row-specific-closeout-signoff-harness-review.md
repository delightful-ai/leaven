# Row-Specific Closeout Signoff Harness Review

Date: 2026-05-24
Reviewer: Darwin read-only reviewer (`019e5d04-9dbd-75a1-ba91-6813e2914321`)

Scope:

- `ps1.harness.negative_denominator`
- Tranche revset: `ouukpqsw` (`public-seam: require row-specific closeout signoff`)
- Files reviewed:
  - `docs/specs/public-seam-v1/conformance-matrix.yaml`
  - `docs/specs/public-seam-v1/reviews/*.md`
  - `crates/leaven-public-seam/src/package/validation.rs`
  - `crates/leaven-public-seam/tests/public_seam_contract/contract_package.rs`

Review method:

- Read-only adversarial semantic review.
- The reviewer was instructed not to edit files, create a workspace/checkout, or
  treat rerunning tests as sign-off.
- The claim to falsify was that proven rows now require row-specific
  adversarial closeout review evidence, while partial/blocker reviews remain
  allowed only as provenance.

Initial findings:

- The first detector admitted whole-file fake passes: a review file could name a
  row and contain unrelated sign-off wording elsewhere.
- Same-row partial reviews could satisfy the gate because the first regression
  used an unrelated review that did not name the row.
- Follow-up detectors still admitted stale blocker reviews with wording such as
  "required before the row can move to proven", "not as sign-off", `proven`
  inside `provenance`, and sign-off sections that kept rows pending.
- The final blocking phrase gap was `remains pending`.

Resolution:

- `audit_conformance_evidence` now requires a proven row to cite at least one
  row-specific closeout review.
- The accepted closeout grammar is deliberately narrow: explicit row closeout
  lines, row bullets in signed-off/final-decision sections, or clean single-row
  verdict documents.
- Partial, pending, blocker, negated sign-off, provenance-substring, and
  pending-row sign-off-section language is rejected as closeout evidence.
- Regression tests cover same-row partial evidence, same-row blocker evidence,
  negated sign-off language, `proven` inside `provenance`, sign-off sections
  that keep rows pending, and `remains pending` variants.

Final decision:

- Sign off on `ps1.harness.negative_denominator` for this harness tranche.
- The reviewer found no remaining fake-pass findings in the current diff and
  matrix corpus.
- The reviewer confirmed that partial/blocker reviews can remain as provenance,
  but cannot be the only closeout evidence for a proven matrix row.

Verification reported by implementer:

- `cargo fmt --check`
- `cargo test -p leaven-public-seam --test public_seam_contract contract_package::conformance -- --nocapture`

Limits:

- This sign-off covers the conformance evidence audit harness. It does not
  independently prove any neighboring runtime row; those rows still depend on
  their own executable evidence and row-specific adversarial sign-off.
