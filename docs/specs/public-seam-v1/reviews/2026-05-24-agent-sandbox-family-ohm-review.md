# Agent and Sandbox Family Review

Date: 2026-05-24
Reviewer: Ohm sub-agent (`019e5bf5-f855-7a70-b109-d67469c0a91e`)

Scope:

- `ps1.agent.contract`
- `ps1.sandbox.exec_streaming`
- Public-seam agent and sandbox execution contract tranche

Review method:

- Read-only adversarial semantic inspection against the locked public-seam V1
  package, row proof fields, fake-pass traps, implementation, tests, and
  public-maturity wording.
- The reviewer was explicitly instructed not to treat rerunning the same tests
  as sign-off.

Initial decision:

- Do not sign off.

Initial blocker:

- `sandbox_exec` capability authority enforced subprocess and filesystem
  execution policy but did not enforce `execution_policy.network`. A capability
  with network denied could still reach `host.sandbox_exec`.

Resolution:

- `validate_sandbox_execution_policy` now denies `sandbox_exec` when capability
  execution policy sets network to `deny`.
- `call_authority_rejects_sandbox_exec_outside_workspace_execution_policy`
  includes the network-denied sandbox negative.
- `plan_execution_with_capability_denies_sandbox_policy_before_host_effects`
  proves the network denial leaves host calls, cached calls, and writes empty.

Final decision:

- Sign off on promoting `ps1.agent.contract`, scoped to public-seam contract
  proof.
- Sign off on promoting `ps1.sandbox.exec_streaming`, scoped to public-seam
  contract proof.

Caveats:

- This is not concrete provider/runtime behavior, ACP process transport,
  concrete sandbox backend execution, or streaming delivery.
- The sign-off did not depend on rerunning tests.
