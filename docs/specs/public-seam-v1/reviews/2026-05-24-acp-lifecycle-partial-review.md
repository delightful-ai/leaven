# ACP Lifecycle Partial Review

Reviewer: Boyle (`019e581f-8955-79b3-a308-182ac5f143d4`)

Scope:

- `ps1.acp.transport_profile`
- `ps1.acp.lifecycle_backpressure`

Decision:

- Acceptable as pending partial evidence.
- No row is signed off for promotion by this review.

Findings:

- No critical findings.
- The tranche adds profile-derived ACP worker session/lifecycle primitives and tests for engine-client/worker-agent role vocabulary, stdio-first session facts, bounded progress-update queues, queue-overflow denial, and post-cancellation update denial.
- Public maturity classification was initially missing for `AcpWorkerSession`, `AcpSessionLifecycle`, `AcpSessionUpdate`, and `AcpSessionCancellation`. Follow-up classified them as advanced public seam contracts for profile-derived lifecycle facts only.
- The primitives do not implement `flow_control.backpressure` strategy-specific behavior. The locked schema admits `pause_worker`, `drop_noncritical_updates`, and `disconnect`; current lifecycle evidence proves bounded rejection/backpressure pressure, not all configured strategy semantics.
- `AcpSessionLifecycle::bounded(...)` is constructible as a Rust primitive without a validated profile. This is acceptable as primitive evidence only because `AcpWorkerSession::start(...)` also exercises the profile-derived route, but it blocks promotion until an ACP-facing lifecycle path is proven.
- Tests passing is supporting evidence only; rerunning the same tests is not review sign-off.

Blocking status:

- Keep `ps1.acp.transport_profile` pending.
- Keep `ps1.acp.lifecycle_backpressure` pending.
- Do not claim integrated ACP transport, process I/O, provider execution, or production lifecycle/backpressure behavior from this tranche.
