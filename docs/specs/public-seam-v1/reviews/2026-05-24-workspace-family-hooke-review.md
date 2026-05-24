# Workspace Family Review

Date: 2026-05-24
Reviewer: Hooke sub-agent (`019e5bd5-1cd5-7fa0-86cf-55b38b2038a9`)

Scope:

- `ps1.workspace.handles_lifecycle`
- Public-seam workspace handle/query/result contract tranche

Review method:

- Read-only adversarial semantic inspection against the locked public-seam V1
  package, row proof fields, fake-pass traps, implementation, tests, and
  public-maturity wording.
- The reviewer was explicitly instructed not to treat rerunning the same tests
  as sign-off.

Initial decision:

- Do not sign off.

Initial blockers:

- No-capability public Plan execution could still run `workspace_query` and
  reach host workspace reads.
- Object-form `WorkspaceRef` values appeared receipt-preimage inconsistent:
  execution collapsed workspace-query scope to a bare id while receipt
  validation hashed the original object ref.

Resolution:

- `execute_plan` now refuses `workspace_query`; workspace reads require
  capability-authorized execution before any host read can run.
- Workspace-query receipt scope and projection now use the full
  `WorkspaceRefFacts::to_value()` representation, preserving `run` and
  `snapshot_fingerprint` in object-form refs.
- `workspace_query_preserves_object_ref_receipt_preimage` proves the object-ref
  route through capability-authorized execution.

Final decision:

- Sign off on promoting `ps1.workspace.handles_lifecycle`, scoped to the
  public-seam contract proof.

Caveats:

- This is not concrete Git backend execution, ACP process/transport behavior,
  watch runtime behavior, provider runtime behavior, or schema-change proof.
- The sign-off did not depend on rerunning tests.
