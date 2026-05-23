# Public Seam V1 Capability Aggregate Budgets Follow-up Review

Scope: `ps1.capability.aggregate_budgets` after the engine-ledger projection was
tightened and delegated runtime cost binding was added.

Reviewed evidence:

- `docs/specs/public-seam-v1/conformance-matrix.yaml`
- `docs/specs/public-seam-v1/02_capability_spec_v0.3.md`
- `docs/specs/public-seam-v1/schemas/leaven.capability.v1.schema.json`
- `crates/leaven-public-seam/src/capability/budget.rs`
- `crates/leaven-public-seam/tests/capability_document.rs`
- `crates/leaven-engine/tests/budget_laws.rs`
- `crates/leaven-public-seam/AGENTS.md`

Result:

- Sign-off for `ps1.capability.aggregate_budgets` under the current row
  wording.

Findings resolved:

- The f64 precision bypass is closed by rejecting runtime projection above
  `9_007_199_254_740_991` and by testing that charging that exact limit plus
  `1` through `BudgetLedger` is denied on the aggregate `usd_micro` axis.
- Delegated work is no longer only a stage-name convention.
  `CapabilityDocument::delegated_runtime_cost` validates parent/child
  attenuation before returning runtime `Cost`, and the negative test charges
  the child work against the parent-derived engine ledger where parent spend
  plus child spend is denied by the parent aggregate cap.

Non-blocking gaps:

- Provider metering, ACP session lifecycle, and durable spend persistence remain
  out of scope for this row as currently written. The row closes at
  `semantic_denial` over the engine budget ledger plus public-seam
  implementation owner, and `crates/leaven-public-seam/AGENTS.md` continues to
  classify provider/session/persistence behavior as not proven by these
  exports.

Reviewer note:

- The follow-up review was read-only and semantic. It did not treat rerunning
  the implementer's tests as proof.
