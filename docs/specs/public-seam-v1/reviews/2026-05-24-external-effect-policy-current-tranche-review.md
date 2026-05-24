# Public Seam V1 External Effect Policy Current Tranche Review

Date: 2026-05-24

Reviewer: Hegel (`019e5978-0e76-7413-8c68-e87e219738c4`)

Reviewed revset: `qxmurqtz::mlrvumqt`

Reviewed commits:

- `qxmurqtz` / `fedfde2b6fe8` - `public-seam: enforce execution policy in call authority`
- `mrvkrrpq` / `643d16623f6a` - `public-seam: bind LM tool result message ids`
- `ontomuwp` / `7103b6cc9b9` - `public-seam: check agent command allow-lists as sets`
- `mlrvumqt` / `907e8802` - `public-seam: close agent tool policy bypass`

## Initial Findings

The first review found one blocker and one major overclaim risk.

- Blocker: schema-valid `agent_run` requests with omitted or empty
  `tool_policy` could pass public-seam authority while `leaven-agent`
  defaulted `AgentToolPolicy` to shell-enabled execution.
- Major risk: `agent_run.tool_policy.allowed_commands` was checked only as a
  declaration. It was not carried in provider-neutral `AgentToolPolicy`, and
  observed agent command records were not checked against it.

## Resolution

The follow-up commit resolved the blocker by moving the primitive to the owner:
`leaven-agent::AgentToolPolicy` now defaults to no shell and carries
`allowed_commands` as provider-neutral request vocabulary.

The public seam now:

- lowers missing `allow_shell` to `false`,
- carries declared `allowed_commands` into `AgentToolPolicy`, and
- rejects observed agent command records whose `argv[0]` is outside declared
  `allowed_commands` when a plan declares that policy.

The command-policy claim is intentionally scoped to provider-neutral request
vocabulary and public-seam result validation. It is not claimed as full
provider enforcement or ACP transport proof.

## Sign-Off

The follow-up review found no blockers for recording the tranche as honest
pending partial evidence. The reviewer did not run tests and did not treat test
replay as sign-off; the review was semantic inspection against the locked spec,
code, tests, and matrix.

No row may be promoted from this tranche.

## Status

Rows remain in their previous statuses. Pending rows remain pending, including:

- `ps1.lm.contract`
- `ps1.agent.contract`
- `ps1.sandbox.exec_streaming`
