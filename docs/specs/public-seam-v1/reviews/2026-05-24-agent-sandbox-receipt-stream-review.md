# Public Seam V1 Agent/Sandbox Receipt And Stream Review

Date: 2026-05-24

Reviewer: Huygens (`019e58e7-e950-7703-838f-aec07005399b`)

Scope:
- `change_from_agent_session` proposal authority binding.
- Completed `sandbox_exec` stdout/stderr blob-ref validation for live host outcomes, replayed Plan Results, and ACP extension results.
- Evidence updates for `ps1.proposal.surface_authority`, `ps1.agent.contract`, `ps1.sandbox.exec_streaming`, and `ps1.acp.extension_results`.

Reviewed claims:
- `change_from_agent_session.effect.agent_receipt` must appear in proposal `read_receipts`, rejecting omitted or mismatched agent session receipts without claiming the runtime produced the session.
- Completed `sandbox_exec` results must preserve `stdout_ref` and `stderr_ref` blob refs across live execution, replay validation, and ACP extension-result validation.
- Matrix status remains honest: the already-proven proposal row only gains evidence; agent, sandbox, and ACP extension-result rows remain pending.

Reviewer result:
- No Critical findings.
- No Important findings.
- Minor finding: `crates/leaven-public-seam/AGENTS.md` under-documented that the sandbox stdout/stderr invariant is also enforced through ACP extension-result validation.

Resolution:
- Updated `crates/leaven-public-seam/AGENTS.md` to state that live host outcomes, replayed Plan Results, and ACP extension results must preserve completed sandbox stdout/stderr blob refs.

Sign-off:
- The reviewer allowed committing the tranche with `ps1.agent.contract`, `ps1.sandbox.exec_streaming`, and `ps1.acp.extension_results` still pending.
- The reviewer explicitly did not treat rerunning tests as sign-off; the review focused on spec drift, fake passes, missing negatives, topology leaks, and public-maturity overclaiming.
