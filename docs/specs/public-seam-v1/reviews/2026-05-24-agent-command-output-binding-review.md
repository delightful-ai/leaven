# Agent Command Output Binding Review

Date: 2026-05-24
Reviewer: Peirce (`019e5ae2-17da-7d81-8c3f-30571ae28933`)

Scope: partial pending-row evidence for `ps1.agent.contract`.

The reviewed claim was not full row closeout. The claim was that
`agent_session` projection now binds observed agent command stdout/stderr and
captured output-file blobs to provider-neutral `leaven_agent::AgentSession`
facts and `leaven_workspace::CommandOutput` bytes before those refs can appear
in a public Plan Result value.

## Initial Findings

Peirce found two blocking issues.

- `PlanAgentRunOutcome::completed().with_commands(...)` was still a public
  route, allowing hosts to hand-write command stdout/stderr/file refs without
  `AgentCommandOutputRefs` byte binding.
- Truncated command stdout/stderr captures could be bound as complete command
  output, while output files already rejected truncation.

Peirce also found incomplete negative coverage for ref-count mismatch, byte
count mismatch, stderr hash mismatch, output-file hash mismatch, stream
truncation, and the public escape hatch itself.

## Resolution

The escape hatch was closed by making `PlanAgentRunOutcome::completed`,
`with_transcript_ref`, `with_commands`, and agent `with_cost` private. Public
host construction now goes through
`PlanAgentRunOutcome::from_agent_session_with_command_output_refs`, with
`with_parsed` left public only for JSON-schema payloads.

The command-output builder now rejects truncated stdout and stderr before blob
binding, matching output-file truncation rejection.

The malformed-result tests no longer use a public unbound builder. They execute
a valid bound agent result, mutate the Plan Result into schema-valid forged
cases, rebind the result hash, and validate the forged result.

`crates/leaven-public-seam/tests/agent_contract.rs::agent_session_command_output_refs_must_bind_captured_bytes_and_files`
covers missing command-output ref sets, wrong stdout hash, wrong stdout byte
count, wrong stderr hash, missing output-file refs, extra output-file refs,
wrong output-file hash, truncated stdout, and truncated stderr.

## Follow-Up Sign-Off

Follow-up review found the prior high and medium findings resolved in
code/tests. It found no topology leak: the public-seam crate is projecting and
validating provider-neutral `leaven_agent::AgentSession` plus
`leaven_workspace::CommandOutput`; it is not taking over runtime/provider
execution.

This is acceptable as partial pending-row evidence only. `ps1.agent.contract`
remains pending. Residual closeout prerequisites include real provider/session
transport proof, complete policy fingerprint semantics, and end-to-end
`ChangeFromAgentSession` proof over the session receipt path.
