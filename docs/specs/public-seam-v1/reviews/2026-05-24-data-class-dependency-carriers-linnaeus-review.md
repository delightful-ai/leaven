# 2026-05-24 Data-Class Dependency Carrier Review

Reviewer: Linnaeus (`019e5b0f-c309-7621-8e6f-efea38160973`)

Scope:

- `crates/leaven-public-seam/src/call_authority.rs`
- `crates/leaven-public-seam/src/plan_execution.rs`
- `crates/leaven-public-seam/src/plan_execution/evaluate.rs`
- `crates/leaven-public-seam/tests/plan_document.rs`
- Matrix row: `ps1.visibility.data_class_propagation`

Prompt notes:

- Passing the parent-run tests was explicitly not sufficient sign-off.
- Review focus was spec drift, fake passes, missing negatives, topology leaks,
  over-broad domain JSON scanning, and public-maturity overclaiming.

Initial findings and resolutions:

- `workspace_listing.entries[].data_classes` was not traversed before host call
  execution. Resolved by collecting entry classes only when the dependency value
  is a `workspace_listing`.
- Literal `Expr.data_classes` were not propagated into call authority because
  literal evaluation returned only `expr.value`. Resolved by carrying binding
  data classes separately through `ExecutionState` and `ResolvedDependencies`
  without mutating the host-visible dependency value.
- `text`, `json`, and `structured` dependency objects could be treated as seam
  carriers based only on `kind` plus `data_classes`. Resolved by requiring
  `visibility` for those output-record-shaped carriers and adding a domain JSON
  guard for `kind: text` without `visibility`.
- `source_refs` should not be treated as data-class carriers; they are `InfoRef`
  values, not `TraceRef` values. The implementation keeps `trace_refs` as the
  traversal point for trace data classes.

Follow-up verdict:

- No blocking findings for adding narrow partial evidence to
  `ps1.visibility.data_class_propagation`.
- The row must remain pending.

Residual limitations:

- This tranche is call-authority evidence only. `binding_data_classes` is passed
  to `execute_call`, but write execution still receives dependency values only.
  This blocks row promotion because the row requires query, call, and write
  propagation.
- Literal `Expr.data_classes` are enforced before live call execution, but are
  not bound into call/write request hashes or receipt replay dependency
  reconstruction. Do not describe this as receipt-bound propagation.

Approved evidence wording:

Partial evidence that capability-scoped call execution denies dependency
data-class drops for literal expression classes and selected nested public-seam
carriers before host call execution.
