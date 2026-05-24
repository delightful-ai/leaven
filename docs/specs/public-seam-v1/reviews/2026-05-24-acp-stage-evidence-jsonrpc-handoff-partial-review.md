# ACP, Stage, and Evidence Partial Tranche Review

Date: 2026-05-24
Reviewer: Hubble (`019e5997-0c65-7693-a986-4a4333a091ad`)
Scope revset: `mlrvumqt..xmnsnztn`

## Reviewed Claim

This review covered a partial public-seam V1 tranche:

- ACP JSON-RPC request/response envelope binding for locked Leaven extension
  methods.
- Reflect/propose handoff validation over the active `reflect_then_propose`
  package shape.
- Evidence-envelope declared data-class coverage for public/private/trace
  projections.

Rows remain pending. This review is not row-promotion evidence for integrated
ACP transport/runtime, runtime stage lowering, evaluator evidence production,
or graph mutation behavior.

## Initial Findings

Hubble found no Critical issues and two Important issues:

- Explicit non-target `data_classes: []` was treated like omission, allowing a
  declared empty top-level data-class set to pass while public/private/trace
  projections carried classes.
- ACP JSON-RPC request/response validators rejected some forbidden fields but
  still allowed arbitrary extra top-level fields, permitting valid Plan IR or
  extension-result envelopes to smuggle private in-process payloads beside the
  locked JSON-RPC members.

Hubble also found two Minor issues:

- Reflect/propose handoff tests did not directly cover all shared-field
  mismatch negatives.
- Pending stage and evidence rows recorded the tranche under ordinary
  closeout-evidence fields instead of `partial_contract_*` fields.

## Resolution

The follow-up commit `xmnsnztn` resolved the findings:

- `EvidenceEnvelopeDocument` now treats top-level `data_classes` field presence
  as a declaration, including explicit empty arrays.
- `evidence_envelope_rejects_declared_non_target_data_class_gaps` covers the
  explicit empty declaration bypass.
- `AcpJsonRpcRequestDocument` and `AcpJsonRpcResponseDocument` now close their
  allowed top-level JSON-RPC member sets.
- `acp_jsonrpc_rejects_in_process_or_cross_method_fakes` covers smuggled
  request and response payloads beside otherwise valid JSON-RPC envelopes.
- `reflect_propose_handoff_rejects_single_prompt_and_stale_reflection_fakes`
  now covers base revision, parent, surface fingerprint, and query-policy
  fingerprint mismatch negatives.
- `ps1.evidence.visibility_receipts` and
  `ps1.stage.reflection_proposal_split` now record this tranche under
  `partial_contract_test_evidence` and
  `partial_contract_implementation_evidence`.

## Follow-Up Verdict

Hubble found no remaining Critical, Important, or Minor findings after the
fixes.

This tranche is signed off as partial evidence only. No row is promoted:

- `ps1.acp.transport_profile` remains pending because JSON-RPC contract
  validation is not ACP process I/O or worker lifecycle runtime proof.
- `ps1.acp.extension_results` remains pending because extension-result envelope
  validation is not integrated transport/runtime proof.
- `ps1.stage.reflection_proposal_split` remains pending because handoff
  validation is not runtime stage lowering or ACP delivery proof.
- `ps1.evidence.visibility_receipts` remains pending because envelope
  validation is not evaluator evidence production, redaction execution, or
  receipt persistence proof.

The reviewer did not use test reruns as sign-off; this was semantic inspection
of the revset, specs, implementation, tests, matrix, and public-maturity docs.

## Main-Agent Verification

- `cargo fmt --check`
- `cargo test -p leaven-public-seam --test acp_profile acp_jsonrpc -- --nocapture`
- `cargo test -p leaven-public-seam --test evidence_envelope declared_non_target -- --nocapture`
- `cargo test -p leaven-public-seam --test stage_payloads reflect_propose_handoff -- --nocapture`
- `cargo test -p leaven-public-seam --test acp_profile -- --nocapture`
- `cargo test -p leaven-public-seam --test evidence_envelope -- --nocapture`
- `cargo test -p leaven-public-seam --test stage_payloads -- --nocapture`
- `cargo test -p leaven-public-seam --test contract_package conformance_matrix_rows_are_unique_honest_and_reference_real_files -- --exact --nocapture`
- `cargo clippy -p leaven-public-seam --tests -- -D warnings`
- `cargo test -p leaven --test topology_contract`
- `cargo test -p leaven-public-seam --tests`
