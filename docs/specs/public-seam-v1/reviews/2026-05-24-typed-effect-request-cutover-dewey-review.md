# Typed Effect Request Cutover Review

Date: 2026-05-24
Reviewer: Dewey (`019e5b54-d6fe-7750-907a-56c27d215966`)

Scope: partial pending-row evidence for `ps1.lm.contract`,
`ps1.agent.contract`, and `ps1.sandbox.exec_streaming`.

The reviewed claim was that the representative public-seam
`PlanExecutionHost` path no longer hands raw Plan call JSON to LM, agent, or
sandbox hosts as the host's source of execution truth. The public-seam owner
now lowers those calls before host execution into:

- `leaven_lm::LmRequest`
- `leaven_agent::AgentRunRequest`
- `leaven_workspace::Command`

This is a fake-pass hardening tranche. It is not provider runtime execution,
ACP delivery, sandbox production streaming, or full row closeout.

## Initial Finding

Dewey found one blocking medium issue in the first review pass:
`agent_run.runtime` was required by the locked Plan IR schema but was not
preserved into the host-visible provider-neutral agent request. That left a
metadata-loss fake pass where a plan could request one runtime while a fixed
host executed another and returned its own runtime fingerprint.

## Resolution

The finding was resolved by moving the missing primitive to the owning neutral
agent layer and enforcing it through the public-seam lowering path:

- `leaven_agent::AgentRunRequest` now carries requested runtime identity and an
  optional expected runtime fingerprint.
- `PlanAgentRunRequest` exposes the lowered runtime facts and no raw
  `agent_run` call JSON.
- `execute_agent_run_call` rejects a host outcome whose runtime fingerprint
  does not match the Plan IR requested fingerprint before receipts are
  recorded.
- Tests now prove runtime selector preservation and fingerprint mismatch
  rejection.

## Follow-Up Result

Dewey found no blocking issues after the fix.

The reviewer specifically confirmed:

- the LM claim is not blocked: `lm_complete` hosts receive a lowered
  `LmRequest`, and multimodal/extension plus streaming-shaped request
  negatives reject before host calls;
- the agent metadata-loss finding is resolved for the representative
  public-seam path;
- the sandbox partial claim is not blocked: hosts receive a lowered
  `Command`, while stream transport/backpressure remains unproven;
- the ACP fixture role change from reflector to proposer is honest because
  mint-time tests still reject runner/reflector target grants;
- matrix updates are honest partial evidence only.

## Residual Risk

`ps1.lm.contract`, `ps1.agent.contract`, and `ps1.sandbox.exec_streaming`
remain pending.

This tranche does not prove live provider execution, provider leaves enforcing
runtime selectors, ACP process/session delivery, runtime-pool authorization,
production sandbox execution, or streaming transport/backpressure.

## Main-Agent Verification

- `cargo test -p leaven-agent --test runtime_contract -- --nocapture`
- `cargo test -p leaven-public-seam --test plan_document -- --nocapture`
- `cargo test -p leaven-public-seam --tests -- --nocapture`
- `cargo clippy -p leaven-agent --tests -- -D warnings`
- `cargo clippy -p leaven-public-seam --tests -- -D warnings`
- `cargo test -p leaven --test topology_contract -- --nocapture`
- `cargo fmt --check`
