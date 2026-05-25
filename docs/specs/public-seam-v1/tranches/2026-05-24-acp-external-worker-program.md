# ACP External Worker Program Tranche

Date: 2026-05-24

Revset: `ltolmsor`

Commit: `2fd8a096 acp: prove multi-call Python worker programs`

Scope:
- Transport owner: `crates/leaven-acp`
- Real benchmark proof owner: `examples/trace2skill_spreadsheetbench`
- Spec surface: `ps1.acp.transport_profile`, `ps1.acp.extension_results`, `ps1.acp.lifecycle_backpressure`, `ps1.acp.no_mcp_v1`
- Evidence only; no matrix row status or evidence fields were changed in this tranche.

Implemented transport proof:
- `crates/leaven-acp/tests/stdio_session_contract.rs::stdio_session_runs_python_external_worker_program_across_v1_method_families`
- `crates/leaven-acp/tests/stdio_session_contract.rs::stdio_session_rejects_external_worker_program_bare_payload_mid_sequence`

Implemented benchmark proof:
- `examples/trace2skill_spreadsheetbench/tests/acp_external_worker.rs::acp_external_python_worker_solves_real_spreadsheetbench_case_and_scores_run`
- `examples/trace2skill_spreadsheetbench/tests/acp_external_worker.rs::acp_external_python_worker_success_without_workbook_does_not_clear_benchmark_run`

Narrow transport behavior claim:
- A long-lived Python worker process can execute V1 ACP stdio calls across the locked extension-result envelope shape and representative method families through the public seam transport.
- The worker validates locked JSON-RPC ids, Plan IR schema version, `no_graph_writes`, capability environment, and rejects MCP-shaped method drift.
- A worker that succeeds once and then returns a bare method-specific payload mid-sequence is rejected by public-seam validation.

Real benchmark behavior claim:
- A Python external worker launched through `leaven-acp` can solve the materialized Trace2Skill SpreadsheetBench-Verified case `13-1` by reading the real `.xlsx` workbook, grouping `RANGES` rows by date and reference, writing the `LISTS` answer sections, and returning a valid `leaven/agent.run` ACP result envelope.
- The host-side Trace2Skill scorer observes the initial workbook score below 1.0 and the external worker output score at 1.0.
- A worker that returns a valid ACP `agent_session` success envelope without writing the output workbook does not clear the benchmark run; scoring fails instead of treating ACP success as benchmark success.
- This remains a deterministic local mechanics proof. It is not live model/provider execution, Trace2Skill skill evolution, analyst/merge execution, or paper metric reproduction.

Verification run before recording:
- `cargo test -p leaven-acp --test stdio_session_contract stdio_session_runs_python_external_worker_program_across_v1_method_families -- --exact --nocapture`
- `cargo test -p leaven-acp --test stdio_session_contract stdio_session_rejects_external_worker_program_bare_payload_mid_sequence -- --exact --nocapture`
- `cargo test -p leaven-acp --test stdio_session_contract -- --nocapture`
- `cargo test -p leaven-public-seam --test public_seam_contract acp_ -- --nocapture`
- `cargo fmt --check`
- `cargo test -p trace2skill_spreadsheetbench --test acp_external_worker -- --nocapture`

Review gate:
- First adversarial review result: no sign-off. The reviewer found the earlier ACP-only Python worker proof was real transport behavior but overclaimed end-user E2E program readiness because Rust precomputed method/result cases and the worker did not execute a real benchmark task.
- Matrix rows remain unchanged until a fresh adversarial read-only review inspects the updated Trace2Skill benchmark proof for spec drift, fake passes, missing negatives, topology leaks, and public-maturity overclaiming.
