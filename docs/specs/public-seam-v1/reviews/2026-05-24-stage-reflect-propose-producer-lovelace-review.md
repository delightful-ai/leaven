# Stage Reflect Propose Producer Lovelace Review

Date: 2026-05-24
Reviewer: Lovelace (`019e5b88-eec3-7c52-882f-a28c54d8e15b`)

Scope:

- `ps1.stage.reflection_proposal_split`
- `ps1.stage.payload_receipts`
- Generic agentic stage lowering and GEPA Git-program reflect/propose producer
  projection into the locked public-seam stage-payload owner.

Reviewed tranche:

- `crates/leaven-agentic/src/public_seam_stage.rs`
- `crates/leaven-agentic/tests/public_seam_stage.rs`
- `crates/leaven-agentic/AGENTS.md`
- `crates/leaven-gepa-agentic-git/src/public_seam_stage.rs`
- `crates/leaven-gepa-agentic-git/tests/git_reflection.rs`
- `crates/leaven-gepa-agentic-git/AGENTS.md`
- `crates/leaven-public-seam/src/stage_payload.rs`
- `crates/leaven-public-seam/tests/stage_payloads.rs`
- `docs/specs/public-seam-v1/conformance-matrix.yaml`

Review method:

- Read-only adversarial semantic review against the locked public-seam specs,
  schemas, conformance-matrix fake-pass traps, current code, tests, and prior
  stage reviews.
- The reviewer was explicitly instructed that rerunning the same tests was not
  sign-off.

Findings:

- Critical: none.
- Important: `ps1.stage.reflection_proposal_split` can be promoted. The GEPA
  bridge proof runs separate reflector and proposer runtime sessions, validates
  the runtime-produced `ReflectionResult`, builds a `ProposeRequest` from that
  exact result, validates handoff receipts and proposal submission binding
  through `leaven-public-seam`, then applies parsed proposals through
  `RunContext`.
- Important: `ps1.stage.payload_receipts` must stay pending. The tranche gives
  real producer proof for the reflector/proposer family, but runner, scorer,
  judge, callback, artifact adapter, and dataset adapter still rely mostly on
  validator-fixture proof. That does not close the row's fake pass: payload
  enums with no provenance or policy binding.
- Minor: `crates/leaven-agentic/AGENTS.md` still said locked seam lowering was
  not yet done; the main rollout updated that wording before relying on the file
  as ownership evidence.

Resolution:

- Promote `ps1.stage.reflection_proposal_split` to `proven`.
- Keep `ps1.stage.payload_receipts` pending.
- The stage-split sign-off covers the reflect/propose producer path and
  submission binding. It does not prove ACP transport, all stage roles, provider
  execution, GEPA search policy, or the broader payload-receipts row.

Verification evidence from main rollout:

- `cargo fmt --check`
- `cargo test -p leaven-agentic --test public_seam_stage -- --nocapture`
- `cargo test -p leaven-agentic -- --nocapture`
- `cargo clippy -p leaven-agentic --tests -- -D warnings`
- `cargo test -p leaven-gepa-agentic-git -- --nocapture`
- `cargo clippy -p leaven-gepa-agentic-git --tests -- -D warnings`
- `cargo test -p leaven-public-seam --test contract_package -- --nocapture`
- `cargo test -p leaven --test topology_contract`
