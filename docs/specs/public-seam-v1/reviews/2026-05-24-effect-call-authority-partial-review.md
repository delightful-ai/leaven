# Public Seam Effect-Call And Authority Partial Review

Date: 2026-05-24T03:06:21Z

Reviewer: Pauli (`019e57e4-9dce-70f2-a90f-eae4c956f291`)

Scope:

- `nqyyskmonzsk` / `6ed970a9bf4f` `public-seam: tighten effect call result contracts`
- `ysulrmqykmxz` / `5702f3c7c724` `public-seam: gate plan execution on call authority`
- `rkmywpvtpxuz` / `a12498231c55` `public-seam: resolve capability execution review blockers`

Rows touched:

- `ps1.visibility.data_class_propagation`
- `ps1.lm.contract`
- `ps1.agent.contract`
- `ps1.sandbox.exec_streaming`

Initial blockers:

- `execute_plan_document_with_capability` rejected valid V1 workspace lifecycle calls because `call_authority` did not map `workspace_materialize` or `workspace_release`.
- The same capability-scoped execution path still allowed `emit_run_event` writes under an LM-only capability.
- `call_authority` did not project enough grant-request dimensions for policy-bound sandbox/agent execution.

Resolution:

- `call_authority` now maps workspace lifecycle calls and carries candidate/workspace resource selectors plus workspace operation dimensions.
- `execution_authority` now gates capability-scoped execution before host effects, including `event.emit` for `emit_run_event` and proposal writes through `proposal_authority`.
- Call authority now carries LM model/purpose/model role, output schema/surface facts, sandbox workspace/op/command/timeout facts, agent workspace/limit facts when present, and workspace lifecycle dimensions.

Follow-up verdict:

No blockers found in the follow-up fix. The tranche is acceptable as pending partial evidence only.

Residual limits:

- Write authority is representative for the current public-seam execution harness, not full V1 write execution.
- `submit_assessments` and `request_evaluation` are not part of this host execution path.
- Sandbox `stream_updates` transport/backpressure remains unproven.
- Parsed JSON-schema outputs are required to be present but are not validated against the requested schema in this tranche.
- Real provider/runtime execution and full data-class propagation through all runtime surfaces remain unproven.

Matrix handling:

Rows stay `pending`. This review is partial evidence, not row sign-off for promotion.
