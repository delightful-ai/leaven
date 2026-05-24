# LM Contract Provider-Neutral Adapter Review

Date: 2026-05-24
Reviewer: Codex sub-agent (`019e5bca-31c8-7130-a39b-4f90341c2fb6`)

Scope:

- `ps1.lm.contract`
- Current working-copy tranche on top of commit `e8659324`
- Public-seam-owned LM adapter path from locked Plan IR to `leaven_lm::Lm`

Review method:

- Read-only adversarial semantic inspection against the locked public-seam
  specs, conformance row proof fields, fake-pass traps, code, tests, and public
  maturity wording.
- The reviewer was explicitly instructed that rerunning the same tests was not
  sign-off.

Findings:

- Critical: none.
- Important: none.
- Minor: VCS state could not be inspected under the reviewer's read-only
  sandbox because `jj st` needed the git import/export lock. The reviewer did
  not use VCS state as sign-off evidence.

Decision:

- Sign off on promoting `ps1.lm.contract` to `proven`, scoped to the
  provider-neutral public-seam runtime contract.

Semantic basis:

- `PlanLmCompleteRequest::execute_with_lm` closes the previous blocker where
  tests and hosts hand-rolled the final adapter from Plan IR to `LmRequest` to
  `PlanLmCompleteOutcome`.
- The adapter calls `impl leaven_lm::Lm` with the seam-lowered `LmRequest`,
  parses JSON-schema output from the typed `LmResponse`, and constructs success
  only through `PlanLmCompleteOutcome::from_lm_response` with the LM runtime
  fingerprint and metered cost.
- The host proof in `crates/leaven-public-seam/tests/lm_contract.rs` delegates
  directly through `PlanLmCompleteRequest::execute_with_lm`, not through
  hand-written response JSON.
- Fake-pass traps are semantically covered: request metadata is lowered before
  provider call, provider hint and tool-result drift reject before call,
  JSON-schema parsed payloads are validated against the inline schema and
  fingerprint, successful values and receipts must carry matching cost,
  result-side tool metadata/tool-result/extension content is rejected, and
  streaming/multimodal request shapes reject rather than silently downgrade.
- No topology leak was found. `leaven-public-seam` owns Plan IR projection and
  result validation while `leaven-lm` owns provider-neutral request/response
  vocabulary and the `Lm` trait.

Caveat:

- This is not live network/provider integration, ACP transport delivery,
  independent provider-metering truth, or streaming runtime proof.
