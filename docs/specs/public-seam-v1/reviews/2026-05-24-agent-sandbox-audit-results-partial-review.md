# Public Seam V1 Agent And Sandbox Audit Result Partial Review

Scope: `ps1.agent.contract` and `ps1.sandbox.exec_streaming` partial
call-result audit validation in `crates/leaven-public-seam`.

Reviewer: Nietzsche (`019e5878-19a8-7bb3-85c2-a3e5b6204693`)

Review mode: read-only adversarial semantic inspection. The reviewer was
explicitly instructed not to treat rerunning the same tests as sign-off.

Reviewed sources:

- `docs/specs/public-seam-v1/manifest.json`
- `docs/specs/public-seam-v1/01_plan_ir_spec_v0.3.md`
- `docs/specs/public-seam-v1/schemas/leaven.plan.v1.schema.json`
- `docs/specs/public-seam-v1/schemas/leaven.plan_result.v1.schema.json`
- `docs/specs/public-seam-v1/conformance-matrix.yaml`
- `docs/specs/public-seam-v1/notes/CONFORMANCE_TESTS_v0.3.md`
- `crates/leaven-public-seam/src/plan_execution/receipts.rs`
- `crates/leaven-public-seam/tests/plan_document.rs`

## Findings And Resolution

1. Initial review found semantic drift: `sandbox_exec` required a `files`
   object even when no artifacts were captured. Resolution: `files` is now
   optional and artifact file paths are validated only when present. A
   schema-valid sandbox result with no `files` is accepted after result-hash
   rebinding.

2. Initial review found decorative command receipts: `agent_run` command
   records only required any string. Resolution: command record receipts must
   match the enclosing agent session receipt; forged command receipts are
   rejected.

3. Initial review found narrow sandbox path coverage. Resolution: sandbox file
   path negatives now cover absolute paths, parent traversal, empty paths, and
   empty components.

## Sign-Off

Follow-up review reported no blocking findings. The tranche is acceptable as
partial pending-row evidence for receipt/value semantic rejection of
schema-valid audit-thin `agent_run` and `sandbox_exec` results.

This does not prove agent provider execution, sandbox backend enforcement,
transport delivery, or full row closeout. `ps1.agent.contract` and
`ps1.sandbox.exec_streaming` remain `pending`.
