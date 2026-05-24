# Agentic Stage Split Review

Date: 2026-05-24
Reviewer: Codex adversarial sub-agent (`019e5b82-baba-7083-9713-c1734df2381d`)

Scope:

- `ps1.stage.reflection_proposal_split`
- Generic `leaven-agentic` reflect-then-propose lowering into the locked
  public-seam stage-payload owner.

Reviewed tranche:

- `crates/leaven-agentic/src/public_seam_stage.rs`
- `crates/leaven-agentic/tests/public_seam_stage.rs`
- `crates/leaven-agentic/AGENTS.md`
- `crates/leaven-agentic/src/{proposer.rs,repairing_proposer.rs,evaluator.rs,artifact_reflector.rs}`
- `crates/leaven-public-seam/src/stage_payload.rs`
- `crates/leaven-public-seam/tests/stage_payloads.rs`
- `docs/specs/public-seam-v1/conformance-matrix.yaml`

Review method:

- Read-only adversarial semantic review against the locked public-seam specs,
  schemas, conformance matrix fake-pass traps, current code, tests, and prior
  stage/submission reviews.
- The reviewer was explicitly instructed that rerunning the same tests was not
  sign-off.

Findings:

- Critical: none.
- Important: none blocking row promotion.
- Minor: `ps1.stage.payload_receipts` should stay pending. The current tranche
  gives enough proof for the reflection/proposal split, but it does not prove
  the broader payload-receipts row across all payload roles.

Resolution:

- Promote `ps1.stage.reflection_proposal_split` to `proven`.
- The sign-off is limited to the separated reflection/proposal stage contract:
  `ReflectRequest`, `ReflectionResult`, and `ProposeRequest` are distinct
  payloads, the proposer consumes the exact fingerprinted `ReflectionResult`,
  stage-call ids and receipts are distinct and bound, and public-seam validators
  reject stale reflection, single-prompt, forged-receipt, and mutation-without-
  handoff fakes.
- This sign-off does not close `ps1.stage.payload_receipts`, ACP delivery, GEPA
  search policy, graph mutation authority, or provider runtime execution.

Recommended evidence move:

- Positive:
  `crates/leaven-agentic/tests/public_seam_stage.rs::agentic_reflect_propose_handoff_lowers_through_locked_public_seam_owner`,
  `crates/leaven-public-seam/tests/stage_payloads.rs::reflect_propose_handoff_binds_distinct_stage_calls_and_exact_reflection_result`,
  and
  `crates/leaven-public-seam/tests/stage_payloads.rs::reflect_propose_submission_binds_proposal_effects_to_cited_handoff`.
- Negative:
  `crates/leaven-agentic/tests/public_seam_stage.rs::agentic_reflect_propose_handoff_rejects_single_prompt_and_stale_reflection_fakes`,
  `crates/leaven-public-seam/tests/stage_payloads.rs::propose_request_rejects_missing_reflection_result_and_change_schema_authority`,
  `crates/leaven-public-seam/tests/stage_payloads.rs::propose_request_rejects_reflection_source_ref_drop`,
  `crates/leaven-public-seam/tests/stage_payloads.rs::reflect_propose_handoff_rejects_single_prompt_and_stale_reflection_fakes`,
  `crates/leaven-public-seam/tests/stage_payloads.rs::reflect_propose_handoff_rejects_missing_or_forged_stage_receipts`,
  and
  `crates/leaven-public-seam/tests/stage_payloads.rs::reflect_propose_submission_rejects_mutation_without_cited_handoff`.
- Implementation:
  `crates/leaven-agentic/src/public_seam_stage.rs::ReflectRequestPayload`,
  `crates/leaven-agentic/src/public_seam_stage.rs::ReflectionResultPayload`,
  `crates/leaven-agentic/src/public_seam_stage.rs::ProposeRequestPayload`,
  `crates/leaven-agentic/src/public_seam_stage.rs::ReflectProposeHandoffPayload`,
  `crates/leaven-public-seam/src/stage_payload.rs::ReflectProposeHandoffDocument::from_schema_valid_values`,
  `crates/leaven-public-seam/src/stage_payload.rs::ReflectProposeSubmissionDocument::from_valid_handoff_and_plan`,
  and `crates/leaven-agentic/AGENTS.md`.

Verification evidence from main rollout before review:

- `cargo test -p leaven-agentic --test public_seam_stage -- --nocapture`
- `cargo test -p leaven-agentic --tests -- --nocapture`
- `cargo clippy -p leaven-agentic --tests -- -D warnings`
- `cargo test -p leaven-public-seam --test stage_payloads -- --nocapture`
