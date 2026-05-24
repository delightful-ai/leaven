# Workspace Query Partial Review

Reviewer: Hume (`019e575b-ab2a-7fa3-ba63-fe113fbc313a`)

Scope:

- `nnvovmtt` / `21d06d70` `public-seam: execute representative workspace queries`
- `znxxmyns` / `7d732ee7` `public-seam: widen workspace query proof shapes`
- `utnrxqrn` / `1696ce2b` `public-seam: audit workspace query result semantics`
- Matrix row: `ps1.workspace.handles_lifecycle`

Decision: signed off as partial evidence only; row remains pending.

Initial finding:

- `validate_plan_execution_result` could accept a forged workspace-query result
  whose receipt hashes were real but whose value branch did not match the Plan
  IR query op. A `read_file` query could provide a schema-valid
  `workspace_listing` value and recompute `result_hash`; schema validation plus
  decorative qrec validation would not catch the mismatch. The same shape could
  hide a `workspace_file` missing `read_file.expected_data_classes`.

Resolution:

- `validate_workspace_query_receipt` now rebuilds the typed `workspace_query`
  request, derives the expected result branch with
  `workspace_query_expected_value_kind`, rejects result values whose `kind` does
  not match the Plan IR op, and checks `workspace_file` results against
  `read_file.expected_data_classes`.
- `plan_execution_result_rejects_workspace_query_value_forgery_with_valid_hashes`
  forges schema-valid values and recomputed hashes for both fake passes and
  expects validation rejection.

Residual gaps before row promotion:

- Real backend execution and policy proof.
- Positive `git_status` proof.
- `stat`, `digest`, and `git_log` remain explicitly unexecuted by the
  representative harness.
- Full artifact/snapshot behavior beyond representative value projection.
- Broader lifecycle denial outside the current public-seam harness.

Reviewer note: tests were not rerun by the reviewer; sign-off is based on
static semantic inspection against the locked V1 spec.
