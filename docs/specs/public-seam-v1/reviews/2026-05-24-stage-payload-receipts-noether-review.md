# Stage Payload Receipts Noether Review

Date: 2026-05-24
Reviewer: Noether (`019e5b94-77fc-7c72-956c-df0b65576607`)

Scope:

- `ps1.stage.payload_receipts`
- Current working-copy tranche adding `leaven-agentic` stage payload
  producer/lowering builders for runner, scorer, judge, callback, artifact
  adapter, and dataset adapter roles.

Reviewed tranche:

- `crates/leaven-agentic/src/public_seam_stage.rs`
- `crates/leaven-agentic/src/lib.rs`
- `crates/leaven-agentic/tests/public_seam_stage.rs`
- `crates/leaven-agentic/AGENTS.md`
- `crates/leaven-public-seam/src/stage_payload.rs`
- `crates/leaven-public-seam/tests/stage_payloads.rs`
- `crates/leaven-gepa-agentic-git/src/public_seam_stage.rs`
- `crates/leaven-gepa-agentic-git/tests/git_reflection.rs`
- `docs/specs/public-seam-v1/conformance-matrix.yaml`

Review method:

- Read-only adversarial semantic review against the locked public-seam specs,
  schemas, current code, tests, matrix evidence, and prior blocker reviews.
- The reviewer was explicitly instructed that rerunning the same tests was not
  sign-off.

Findings:

- Critical: none.
- Important: none blocking row promotion.
- Important: the tranche now has real `leaven-agentic` producer/lowering
  builders for the remaining stage payload roles, not only public-seam fixture
  validation. Runner, scorer, judge, callback, artifact adapter, and dataset
  adapter payloads are built through `leaven-agentic`, then validated through
  the locked `leaven-public-seam` owner.
- Important: the negative proof rejects the fake-pass families named by the
  row: target leakage, missing assessed-output classes, missing payload schema,
  missing capability, stale reflect/propose binding, missing
  receipts/provenance, and proposal effects outside `allowed_effects`.
- Minor: the sign-off does not prove ACP delivery or concrete provider runtime
  execution, and the row does not require those.
- Minor: the earlier Lovelace review is stale for this row because it described
  runner, scorer, judge, callback, and adapter proof as mostly validator
  fixtures. This review supersedes that point for `ps1.stage.payload_receipts`.

Resolution:

- Promote `ps1.stage.payload_receipts` to `proven`.
- Keep the claim scoped to stage payload producer/lowering and public-seam
  semantic validation. Do not treat this as ACP transport, provider execution,
  graph mutation authority, or ordinary facade/default-feature maturity proof.

Verification evidence from main rollout:

- `cargo fmt --check`
- `cargo test -p leaven-agentic --test public_seam_stage -- --nocapture`
- `cargo test -p leaven-agentic --tests -- --nocapture`
- `cargo clippy -p leaven-agentic --tests -- -D warnings`
