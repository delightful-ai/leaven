# ACP Extension And Workspace Query Partial Follow-Up Review

Reviewer: Gibbs (`019e57be-143f-78c2-b036-d629720c346e`)

Scope:

- `ps1.acp.extension_results`
- `ps1.workspace.handles_lifecycle`
- Stage source-ref wording touched while fixing the tranche

Decision:

- Acceptable as pending partial evidence.
- No row is signed off for promotion by this review.

Findings:

- No blocking findings for the tranche.
- ACP extension result hash binding is now enforced by the ACP envelope owner even when the locked primary schema cannot carry a `receipt` field. `validate_acp_extension_result_document` still skips full PlanResult semantic validation for generic `kind: extension` primaries, but `AcpExtensionResultDocument::from_value` now always recomputes the expected JCS `result_hash` for the method's expected receipt category and primary value.
- The new ACP negative test covers forged result hashes for generic extension primaries and receiptless workspace snapshot/listing/diff primaries.
- Stage source-ref wording is now honest: top-level source refs back the diagnosis, and optional nested diagnosis `source_refs` must be non-empty when present. The code does not require every nested diagnosis item to carry `source_refs`, and the AGENTS wording no longer claims that.
- Workspace query evidence remains broad-family partial evidence. `stat` maps to `workspace_listing`, `digest` to `workspace_snapshot`, and `git_log` to `workspace_diff`. Stat now has path/cardinality checks; digest now has algorithm/workspace checks; git_log remains broad-family only.

Non-blocking concerns:

- The locked `workspace_snapshot` schema has no path field, so digest cannot bind the result to the requested path without a future schema/binding primitive. Keep this row pending.
- A direct `validate_plan_execution_result` forged-result fixture for stat/digest would strengthen the receipt-validation route beyond the existing execution-route negative.

Follow-up after review:

- Added direct `validate_plan_execution_result` forged-result coverage for stat wrong-path and digest wrong-algorithm/wrong-workspace with recomputed valid result hashes in `crates/leaven-public-seam/tests/plan_document.rs::plan_execution_result_rejects_workspace_query_value_forgery_with_valid_hashes`.

Verification recorded by parent thread:

- `cargo fmt --check`
- `cargo test -p leaven-public-seam --test acp_profile`
- `cargo test -p leaven-public-seam --test plan_document workspace_query -- --nocapture`
- `cargo test -p leaven-public-seam --test stage_payloads`
- `cargo test -p leaven-public-seam --test contract_package conformance_matrix_rows_are_unique_honest_and_reference_real_files -- --exact`
- `cargo test -p leaven-public-seam`
- `cargo clippy -p leaven-public-seam --tests -- -D warnings`
- `cargo test -p leaven --test topology_contract`
