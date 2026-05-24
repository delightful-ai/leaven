# Public Seam V1 Stage Submission Binding Aquinas Review

Date: 2026-05-24
Reviewer: Aquinas (`019e5afe-72c9-7671-a906-364747be0b2d`)

Scope:
- `ps1.stage.reflection_proposal_split`
- `ps1.stage.payload_receipts`

Reviewed tranche:
- Public-seam composite validation for proposal submissions citing a validated
  reflect/propose handoff.
- `ReflectProposeSubmissionDocument` and
  `PublicSeamPackage::validate_reflect_propose_submission_document`.
- Positive and negative tests in `crates/leaven-public-seam/tests/stage_payloads.rs`.

Review method:
- Read-only adversarial semantic review against the locked public-seam specs,
  schemas, conformance-matrix fake-pass traps, current code, tests, and AGENTS
  boundary docs.
- The reviewer was explicitly instructed that rerunning the same tests was not
  sign-off.

Initial findings:
- Critical: none.
- Important: proposal `informed_by` was initially checked by arbitrary nested
  literal string containment. That proved only mention of the proposer receipt
  and would be a fake pass if used as durable provenance or row closeout.
- Important: the package-level API doc said the handoff "authorized" proposal
  writes even though the validator checks cited stage handoff plus allowlists,
  not capability authorization or runtime proposal authority.
- Important: the new crate-root export needed explicit advanced partial-evidence
  maturity framing.

Resolution:
- Stage provenance now requires an exact top-level string in a literal array,
  and a nested-object receipt fake is rejected.
- Proposal submissions now preserve the embedded `ReflectionResult` read
  receipts, include the reflected parent candidate in `causal.inputs`, keep
  change effects within `ProposeRequest.allowed_effects`, keep change schemas
  within `allowed_change_schemas`, match the reflected target/surface, and
  require `change_from_agent_session` proposals to carry their agent receipt in
  proposal `read_receipts`.
- Public docs and AGENTS wording now describe cited-handoff validation rather
  than authorization.

Follow-up sign-off:
- Critical: none.
- Important: none remaining for partial pending-row evidence.
- Minor: none blocking after wording cleanup.

Non-closeout notes:
- Matrix rows remain pending.
- This is public-seam document validation, not runtime lowering through distinct
  reflect/propose execution steps, ACP delivery, provider execution, RunContext
  graph mutation, proposal application, typed schema-level stage provenance, or
  receipt existence/persistence against a runtime receipt store.

Verification evidence from main rollout:
- `cargo test -p leaven-public-seam --test stage_payloads -- --nocapture`
- `cargo fmt --check`
- `cargo clippy -p leaven-public-seam --tests -- -D warnings`
