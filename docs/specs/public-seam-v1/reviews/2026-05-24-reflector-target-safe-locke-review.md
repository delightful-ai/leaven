# Reflector Target Safe Locke Review

Date: 2026-05-24
Reviewer: Locke (`019e5b9d-41c6-7cf0-b16d-f20625ef6169`)

Scope:

- `ps1.visibility.reflector_target_safe`
- Producer-side target-safe `ReflectRequest` lowering plus public-seam semantic
  denial for reflector target leaks.

Reviewed tranche:

- `crates/leaven-agentic/src/public_seam_stage.rs`
- `crates/leaven-agentic/tests/public_seam_stage.rs`
- `crates/leaven-agentic/AGENTS.md`
- `crates/leaven-public-seam/src/stage_payload.rs`
- `crates/leaven-public-seam/tests/stage_payloads.rs`
- `crates/leaven-public-seam/src/call_authority.rs`
- `crates/leaven-public-seam/tests/call_authority.rs`
- `docs/specs/public-seam-v1/conformance-matrix.yaml`

Review method:

- Read-only adversarial semantic review against the locked public-seam specs,
  current code, tests, matrix fake-pass trap, and public maturity docs.
- The reviewer was explicitly instructed that rerunning the same tests was not
  sign-off.

Findings:

- Critical: none.
- Important: none.
- Minor: none.

Resolution:

- `ReflectRequestPayload::new` now rejects recursive `case.target` markers in
  examples before emitting `target_safe_projection` payloads.
- Public-seam validation independently rejects `case.target` leakage in
  reflector source refs, input, output, feedback, side info, score, and data
  classes.
- Call authority denies reflector LM calls carrying `case.target` input classes.
- Public maturity remains scoped to advanced crate-root builder contracts, not
  prelude/default facade exposure.

Sign-off:

- `ps1.visibility.reflector_target_safe` is promotable to `proven`.
- This sign-off does not prove ACP transport, provider execution, runtime prompt
  rendering, or graph mutation authority.

Verification evidence from main rollout:

- `cargo fmt --check`
- `cargo test -p leaven-agentic --test public_seam_stage -- --nocapture`
- `cargo clippy -p leaven-agentic --test public_seam_stage -- -D warnings`
