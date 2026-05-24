# ACP Costful Primary Receipt Follow-Up Review

Date: 2026-05-24
Reviewer: Sagan (`019e5918-2ae6-7392-b2f4-d377a2a47056`)
Scope: partial evidence for `ps1.acp.extension_results`,
`ps1.lm.contract`, `ps1.agent.contract`, and
`ps1.sandbox.exec_streaming`.

## Reviewed Change

- `leaven/lm.complete`, `leaven/agent.run`, and `leaven/sandbox.exec` ACP
  extension primaries now use the same `primary.receipt` identity check before
  cost or shape audit.
- LM ACP extension results now reject a primary whose `receipt` points at a
  different carried receipt than the method-selected call receipt, even when
  both receipts are present and the method-selected receipt hash is rebound to
  the forged primary value.

## Initial Finding

Important: LM ACP extension primaries bound primary cost to the
method-selected call receipt, but did not require the primary's own `receipt`
field to name that same receipt. A forged envelope could therefore carry:

- an expected `lm_complete` receipt whose hash and cost matched the primary;
- a primary whose `receipt` named another carried receipt; and
- that alternate receipt in the envelope.

That weakened the tranche evidence for ACP primary-kind/receipt binding and LM
cost receipt binding.

## Resolution

`validate_effect_primary_audit` now calls `validate_effect_primary_receipt`
for LM, agent, and sandbox effect primaries. The helper requires
`primary.receipt` and rejects any value that differs from the receipt selected
by the method contract.

`acp_extension_results_reject_lm_cost_audit_gaps` now includes the adversarial
LM case from the finding: it mutates `primary.receipt` to `lmrec_other`, keeps
the expected `lm_complete` receipt as `lmrec_acp`, rebinds the expected receipt
hash to the forged primary, carries the alternate receipt, and expects
rejection. This avoids a fake negative that would fail only because the
`result_hash` was stale.

## Sign-Off

Critical: none.
Important: none remaining.
Minor: none.

The reviewer signed off the resolution of the ACP LM primary receipt identity
finding for the seam-local ACP extension-result validator. This supports
pending-row partial evidence that costful ACP extension primaries for LM,
agent, and sandbox bind primary receipt identity, primary cost, and the
method-selected call receipt consistently at the public-seam contract layer.

Rows remain pending. This does not prove ACP transport execution, provider
runtime behavior, sandbox process isolation, full agent proposal semantics, or
data-class propagation outside the already reviewed seam-local value paths.

## Verification Evidence

- `cargo fmt --check`
- `cargo test -p leaven-public-seam --test acp_profile acp_extension_results_reject_lm_cost_audit_gaps -- --exact --nocapture`
- `cargo test -p leaven-public-seam --test acp_profile acp_extension_results -- --nocapture`
- `cargo test -p leaven-public-seam --test contract_package conformance_matrix_rows_are_unique_honest_and_reference_real_files -- --exact --nocapture`
- `cargo clippy -p leaven-public-seam --tests -- -D warnings`
