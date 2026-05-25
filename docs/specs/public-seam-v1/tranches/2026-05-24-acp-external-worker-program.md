# ACP External Worker Program Tranche

Date: 2026-05-24

Revset: `ltolmsor`

Commit: `2fd8a096 acp: prove multi-call Python worker programs`

Scope:
- Owner: `crates/leaven-acp`
- Spec surface: `ps1.acp.transport_profile`, `ps1.acp.extension_results`, `ps1.acp.lifecycle_backpressure`, `ps1.acp.no_mcp_v1`
- Evidence only; no matrix row status or evidence fields were changed in this tranche.

Implemented proof:
- `crates/leaven-acp/tests/stdio_session_contract.rs::stdio_session_runs_python_external_worker_program_across_v1_method_families`
- `crates/leaven-acp/tests/stdio_session_contract.rs::stdio_session_rejects_external_worker_program_bare_payload_mid_sequence`

Behavior claim to review later:
- A long-lived Python worker process can execute a V1 ACP stdio program across graph, workspace, LM, agent, sandbox, proposal, assessment, and workspace-release method families through the public seam transport.
- The worker validates locked JSON-RPC ids, Plan IR schema version, `no_graph_writes`, capability environment, and rejects MCP-shaped method drift.
- A worker that succeeds once and then returns a bare method-specific payload mid-sequence is rejected by public-seam validation.

Verification run before recording:
- `cargo test -p leaven-acp --test stdio_session_contract stdio_session_runs_python_external_worker_program_across_v1_method_families -- --exact --nocapture`
- `cargo test -p leaven-acp --test stdio_session_contract stdio_session_rejects_external_worker_program_bare_payload_mid_sequence -- --exact --nocapture`
- `cargo test -p leaven-acp --test stdio_session_contract -- --nocapture`
- `cargo test -p leaven-public-seam --test public_seam_contract acp_ -- --nocapture`
- `cargo fmt --check`

Review gate:
- Not requested yet. The row matrix remains unchanged until an adversarial read-only review inspects this tranche for spec drift, fake passes, missing negatives, topology leaks, and public-maturity overclaiming.
