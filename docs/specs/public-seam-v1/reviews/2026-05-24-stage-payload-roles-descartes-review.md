# Stage Payload Roles Descartes Review

Date: 2026-05-24
Reviewer: Descartes (`019e5b94-f9d6-7200-9f5a-8abcf0289d5c`)

Scope:

- `ps1.stage.payload_receipts`
- Generic agentic producer-side stage payload builders for runner, scorer,
  judge, callback, artifact adapter, and dataset adapter roles.

Reviewed tranche:

- `crates/leaven-agentic/src/public_seam_stage.rs`
- `crates/leaven-agentic/src/lib.rs`
- `crates/leaven-agentic/tests/public_seam_stage.rs`
- `crates/leaven-agentic/AGENTS.md`
- `crates/leaven-public-seam/src/stage_payload.rs`
- `crates/leaven-public-seam/tests/stage_payloads.rs`
- `docs/specs/public-seam-v1/conformance-matrix.yaml`

Review method:

- Read-only adversarial semantic review against the locked public-seam specs,
  schemas, conformance-matrix fake-pass traps, current code, tests, and
  evidence.
- The reviewer was explicitly instructed that rerunning the same tests was not
  sign-off.

Findings:

- Critical: none.
- Important: the code/test semantics were sufficient for the row, but the
  matrix evidence was stale. It still cited only the older reflect/propose
  producer tests and older implementation symbols, so promoting the row without
  adding the new producer proof would overclaim.
- Important: `crates/leaven-agentic/AGENTS.md` had an internal contradiction.
  The Map/Public Maturity sections correctly classified the new crate-root
  exports as advanced and non-prelude, while Local Bait still described only
  reflect/propose lowering and said full payload-role closeout was not done.
- Minor: none.

Resolution:

- Add the new agentic role-payload positive and negative tests to
  `ps1.stage.payload_receipts` evidence.
- Add the new runner, scorer, judge, callback, artifact adapter, and dataset
  adapter builder symbols to implementation evidence.
- Update `crates/leaven-agentic/AGENTS.md` Local Bait to say the crate owns
  producer-side lowering for all stage payload roles while still not proving ACP
  transport, provider execution, or graph mutation authority.

Sign-off:

- After the resolution above, `ps1.stage.payload_receipts` is promotable to
  `proven`.
- This sign-off does not prove ACP transport, graph mutation, provider runtime
  execution, GEPA search policy, or ordinary facade/prelude exposure.

Verification evidence from main rollout:

- `cargo fmt --check`
- `cargo test -p leaven-agentic --test public_seam_stage -- --nocapture`
- `cargo clippy -p leaven-agentic --test public_seam_stage -- -D warnings`
- `cargo clippy -p leaven-agentic --tests -- -D warnings`
- `cargo test -p leaven-public-seam --test stage_payloads -- --nocapture`
- `cargo test -p leaven-public-seam --test contract_package -- --nocapture`
- `cargo test -p leaven --test topology_contract`
