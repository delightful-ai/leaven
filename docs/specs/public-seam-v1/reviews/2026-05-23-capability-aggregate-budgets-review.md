# Public Seam V1 Capability Aggregate Budgets Review

Scope: `ps1.capability.aggregate_budgets` after public-seam aggregate budget ledger evidence was added.

Reviewed evidence:

- `docs/specs/public-seam-v1/conformance-matrix.yaml`
- `docs/specs/public-seam-v1/02_capability_spec_v0.3.md`
- `docs/specs/public-seam-v1/schemas/leaven.capability.v1.schema.json`
- `crates/leaven-public-seam/src/capability.rs`
- `crates/leaven-public-seam/tests/capability_document.rs`
- `crates/leaven-public-seam/AGENTS.md`

Result:

- No sign-off. `ps1.capability.aggregate_budgets` must remain pending.

Blocking findings:

- The current evidence proves only a public-seam `CapabilityBudgetLedger` helper over one resolved capability document. The matrix row requires enforcement across LM, agent, sandbox, evaluator, and delegated work through the runtime budget owner.
- The tests show aggregate and role totals plus concurrent-call limits on the helper, but they do not execute mixed runtime call types or nested/delegated work through ACP/session permission handling.
- `crates/leaven-public-seam/tests/capability_document.rs::aggregate_budget_ledger_counts_role_spend_against_total_budget` proves role-specific usage cannot bypass the aggregate total cap.
- The public-seam crate maturity text still correctly says aggregate budget spending must land in the owning engine/agent/workspace/LM/evaluator runtime crates before integrated behavior is claimed.

Allowed matrix/document updates:

- Keep `ps1.capability.aggregate_budgets` pending while citing the helper tests and this blocker review as partial evidence.

Disallowed updates:

- Do not mark `ps1.capability.aggregate_budgets` proven from public-seam helper tests alone.
- Do not cite this review as runtime, ACP, provider, or delegated-work budget proof.
