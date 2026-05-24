# ACP Stdio Launch Partial Review

Date: 2026-05-24
Reviewer: Rawls (`019e5933-f283-7c22-a35e-89f4d35dbe8b`)
Scope: partial evidence for `ps1.acp.transport_profile`.

## Reviewed Change

- The validated ACP profile exposes the locked stdio launch environment names:
  `LEAVEN_CAPABILITY_TOKEN`, `LEAVEN_ENDPOINT`, and
  `LEAVEN_CAPABILITY_FINGERPRINT`.
- `AcpStdioWorkerLaunch` models the stdio worker launch environment for a
  validated ACP worker session.
- Worker env construction carries token, endpoint, and expected capability
  fingerprint.
- Artifact-safe launch facts omit the bearer token, and validation rejects
  persisted bearer leakage.
- `AcpStdioWorkerLaunch` is classified as an advanced public seam contract in
  the seam crate's public-maturity ledger.

## Findings And Resolution

Initial important finding: `validate_artifact_env` only rejected the exact
`LEAVEN_CAPABILITY_TOKEN` key, so artifacts could still persist the bearer
secret under another key or inside a composite value.

Resolution: artifact validation now rejects the token env key and any artifact
value containing the bearer token. Tests cover renamed token keys, bearer
header values, and composite command/header strings.

Initial important finding: `AcpStdioWorkerLaunch` derived `Debug`, which would
print the stored bearer token and worker env token value.

Resolution: `AcpStdioWorkerLaunch` now has a custom `Debug` implementation
that redacts the stored bearer token and the token value inside `worker_env`.
The test asserts debug output omits the token and contains `<redacted>`.

Initial public-maturity finding: the new crate-root export was not classified
in `crates/leaven-public-seam/AGENTS.md`.

Resolution: `AcpStdioWorkerLaunch` is now documented as an advanced public
seam contract for locked stdio ACP launch environment facts only, explicitly
not stdio JSON-RPC I/O, process spawning, provider execution, session
supervision, or full ACP transport.

## Sign-Off

Critical: none.
Important: none remaining.
Minor: none.

The reviewer signed off this tranche as partial evidence for
`ps1.acp.transport_profile`: profile-owned stdio launch env constants, required
launch facts, worker env construction, artifact bearer redaction, and public
route maturity.

`ps1.acp.transport_profile` remains pending. This does not prove ACP process
startup, stdio JSON-RPC I/O, provider execution, session supervision, or full
transport behavior.

## Verification Evidence

- `cargo fmt --check`
- `cargo test -p leaven-public-seam --test acp_profile acp_stdio_worker_launch -- --nocapture`
- `cargo test -p leaven-public-seam --test acp_profile -- --nocapture`
- `cargo test -p leaven-public-seam --test contract_package public_seam_routes_reject_ordinary_facade_leaks -- --exact --nocapture`
- `cargo test -p leaven-public-seam --test contract_package conformance_matrix_rows_are_unique_honest_and_reference_real_files -- --exact --nocapture`
- `cargo clippy -p leaven-public-seam --tests -- -D warnings`
- `cargo test -p leaven --test topology_contract`
