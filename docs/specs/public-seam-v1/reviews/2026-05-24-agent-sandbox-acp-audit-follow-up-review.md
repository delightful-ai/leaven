# Agent/Sandbox ACP Audit Follow-Up Review

Date: 2026-05-24

Tranche revset:
- `ykyquuvmvuws`

Scope:
- `ps1.acp.extension_results`
- `ps1.agent.contract`
- `ps1.sandbox.exec_streaming`

Reviewed tranche:
- `agent_run` result validation now rejects schema-valid command records
  without `argv` or `status`, while still requiring command receipts to match
  the enclosing session receipt.
- ACP `leaven/agent.run` extension results reuse the seam-local agent session
  audit checks: transcript blob ref, non-empty command records, command argv and
  status facts, command/session receipt binding, cost, and primary receipt
  binding.
- ACP `leaven/sandbox.exec` extension results reuse the seam-local sandbox
  audit checks: cost, completed exit code, safe relative output-file paths, and
  primary receipt binding.
- Matrix entries were updated only as partial evidence; all reviewed rows remain
  `pending`.

Initial adversarial findings:
- Important: the existing ACP unbound receipt negative could fail on
  data-class coverage before receipt binding, so it was not isolated evidence
  for receipt mismatch rejection.
- Minor: ACP sandbox negatives did not cover primary receipt mismatch, parent
  traversal, empty path, or empty path component cases.
- Minor: shared validator visibility remains internal because `plan_execution`
  is a private module and the crate root does not re-export these functions.

Resolution:
- The unbound receipt negative now carries the needed `transcript.raw` data
  class and avoids mutating the primary after result-hash construction.
- Agent and sandbox primary receipt mismatch negatives now carry an extra
  receipt bound to the primary value so generic missing-receipt validation does
  not preempt the ACP effect-primary audit path.
- Sandbox ACP audit negatives now cover absolute paths, parent traversal, empty
  paths, and empty path components.

Reviewer follow-up:
- Critical: none.
- Important: none.
- Minor: none.
- Semantic sign-off was granted for this tranche as partial evidence only.
  The reviewer explicitly did not treat rerunning tests as sign-off.

Non-closeout notes:
- This review does not prove full ACP delivery, agent provider execution,
  sandbox backend enforcement, or row closeout.
- The reviewed matrix rows remain pending.
