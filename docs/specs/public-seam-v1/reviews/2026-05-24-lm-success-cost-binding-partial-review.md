# LM Success Cost Binding Partial Review

Date: 2026-05-24
Reviewer: Goodall (`019e58db-9f6e-7493-a510-8c3bbef0521a`)
Scope: `ps1.lm.contract` partial evidence only.

## Reviewed Change

- Successful `lm_complete` host outcomes must now carry cost before the public
  seam records an `lm_response` value.
- Successful `lm_response` values carry the cost, and the successful call
  receipt carries the same cost.
- Plan Result replay validation rejects missing LM value cost, missing receipt
  cost, and mismatched value/receipt cost.

## Findings And Resolution

Initial minor finding: live and replay negatives covered the important preimage
tricks, but there was no dedicated cached-hit missing-cost negative.

Resolution: `plan_execution_modes_require_cached_uses_cache_and_refuses_live_misses`
now also covers a cached hit whose outcome omits cost, proves it is rejected,
and asserts no live LM call ran.

## Sign-Off

Critical: none.
Important: none.
Minor: none remaining after the cached-hit regression.

The reviewer signed off this tranche as partial evidence only for
`ps1.lm.contract`.

Semantic basis:

- Requiring successful LM value/receipt cost is semantic Plan Result
  validation; the locked schemas still leave cost optional.
- Live and cached successes converge through the same public-seam outcome
  recorder before cost validation.
- Failed LM calls remain on the existing failed-call cost/charge-receipt path.
- The conformance row remains pending and this does not claim provider runtime,
  streaming, or ACP transport proof.

## Verification Evidence

- `cargo fmt --check`
- `cargo test -p leaven-public-seam --test plan_document lm_complete -- --nocapture`
- `cargo test -p leaven-public-seam --test plan_document plan_execution_modes_require_cached_uses_cache_and_refuses_live_misses -- --nocapture`
- `cargo test -p leaven-public-seam --test plan_document plan_execution_produces_failed_paid_lm_call_and_charge_receipts -- --nocapture`
- `cargo test -p leaven-public-seam --test plan_result plan_result_rejects_failed_call_costs_without_charge_receipts -- --nocapture`
- `cargo test -p leaven-public-seam --test contract_package conformance_matrix_rows_are_unique_honest_and_reference_real_files -- --nocapture`
- `cargo clippy -p leaven-public-seam --tests -- -D warnings`
