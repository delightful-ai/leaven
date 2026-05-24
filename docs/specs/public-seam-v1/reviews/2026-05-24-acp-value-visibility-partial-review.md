# ACP Value Visibility Partial Review

Date: 2026-05-24T04:41:15Z
Reviewer: Godel (`019e5844-ae15-73b0-8408-f6c0b37efb56`)
Scope: current jj working-copy tranche after `zzusknuk 26a6c3ad`.

## Reviewed Claim

The tranche adds public-seam validation that top-level Plan Result value
`trace_refs` and blob-ref-bearing fields (`blob_ref`, `transcript_ref`,
`stdout_ref`, `stderr_ref`, and `files` entries) are covered by the enclosing
value's `data_classes`. ACP extension-result tests cover agent transcript refs
and sandbox stdout/stderr/file refs through the public seam envelope.

The affected matrix rows remain pending. This review is only sign-off for
recording the tranche as partial evidence.

## Initial Finding

Important: the first version of
`plan_result_rejects_value_trace_and_blob_ref_data_class_gaps` mutated a
hash-bound fixture by adding `values.rows.trace_refs` without rebinding result
hashes. That could fail on stale receipt hash binding before exercising the new
visibility/data-class rule, so the test was a fake-pass risk for the
top-level trace-ref negative.

Resolution: the test now rebinds the mutated trace fixture before validation
and asserts the error string includes the nested visibility data-class error
for `transcript.raw`. The blob-ref negative also asserts the corresponding
error for `workspace.file`.

## Follow-Up Verdict

Godel's follow-up review found no Critical, Important, or Minor findings. The
prior blocker is resolved. The tranche is signed off for recording as partial
evidence with the affected rows still pending, and there is no recommendation
to promote any row to proven.

## Verification

Main-agent verification after the fix:

- `cargo fmt --check`
- `cargo test -p leaven-public-seam --test plan_result plan_result_rejects_value_trace_and_blob_ref_data_class_gaps -- --nocapture`
- `cargo test -p leaven-public-seam --test acp_profile acp_extension_results_reject_agent_and_sandbox_blob_ref_data_class_gaps -- --nocapture`
