# ACP Stdio Transport Review

Date: 2026-05-24
Reviewer: Heisenberg sub-agent (`019e5c0c-dc6c-7382-989f-259cfe53fb1a`)
Scope: `ps1.acp.transport_profile`, `ps1.acp.extension_results`,
`ps1.acp.lifecycle_backpressure`

## Inputs Reviewed

- `docs/specs/public-seam-v1/manifest.json`
- `docs/specs/public-seam-v1/goal-readiness-gate.yaml`
- `docs/specs/public-seam-v1/conformance-matrix.yaml`
- `docs/specs/public-seam-v1/profiles/leaven_acp_profile_v1_v0.3.md`
- `docs/specs/public-seam-v1/00_architecture_judgment_v0.3.md`
- `docs/specs/public-seam-v1/notes/CONFORMANCE_TESTS_v0.3.md`
- `crates/leaven-acp/**`
- `crates/leaven-public-seam/AGENTS.md`
- Root and `crates/` topology docs/tests touched by the new crate.

## Initial Findings

The reviewer signed off `ps1.acp.transport_profile` and
`ps1.acp.extension_results` after semantic inspection. The review explicitly
did not treat rerunning tests as sufficient. It found that the new
`leaven-acp` crate starts a real child process, binds ACP profile roles,
passes the launch environment, sends locked JSON-RPC requests over stdio, and
validates all extension-result envelopes through `leaven-public-seam`.

The reviewer initially refused to sign off `ps1.acp.lifecycle_backpressure`
because cancellation could not interrupt a pending `call_extension`, and
backpressure was only a local queue error. Those findings blocked row
promotion until fixed.

## Fixes Reviewed

- Added `AcpStdioCancellationHandle` so recorded cancellation can write
  `session/cancel` while `call_extension` is blocked on worker stdout.
- Added `read_next_session_update` so progress updates can be handled through
  lifecycle control without requiring a normal extension response.
- On disconnect backpressure, the transport writes `session/cancel` to the
  live overproducing worker with the lifecycle cancellation receipt/error.
- Added a late-success guard: after a cancellation is recorded, a worker that
  still emits a schema-valid extension success response is refused as
  `AcpTransportError::Cancelled`.

## Final Verdict

- `ps1.acp.transport_profile`: SIGN OFF.
- `ps1.acp.extension_results`: SIGN OFF.
- `ps1.acp.lifecycle_backpressure`: SIGN OFF after the follow-up fixes.

## Caveats

This sign-off proves Leaven's current hot stdio JSON-RPC ACP transport binding
and lifecycle semantics through black-box subprocess tests. It does not claim
official `agentclientprotocol/rust-sdk` compatibility, provider runtime
execution, or non-stdio transport behavior. The current `session/update` and
`session/cancel` method spelling is the `leaven-acp` transport binding until a
future SDK migration is approved.
